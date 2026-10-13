//! Historial del servidor: el sampler mide cada 2 segundos y acá se resume cada minuto en una
//! fila de `server_samples` (promedios del minuto). Se guardan 30 días.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use ts_rs::TS;

use super::Sample;

/// Cuánto historial se guarda.
const KEEP_SECS: i64 = 30 * 24 * 3600;

/// Lo que se va juntando durante un minuto.
#[derive(Default)]
pub struct MinuteTotals {
    minute: i64,
    count: u32,
    cpu_kinora: f64,
    cpu_ffmpeg: f64,
    cpu_system: f64,
    memory_kinora: i64,
    memory_ffmpeg: i64,
    memory_used: i64,
    memory_total: i64,
    bytes_sent: i64,
    viewers: i64,
}

/// Una fila de `server_samples`.
pub struct MinuteRow {
    minute: i64,
    cpu_kinora: f64,
    cpu_ffmpeg: f64,
    cpu_system: f64,
    memory_kinora: i64,
    memory_ffmpeg: i64,
    memory_used: i64,
    memory_total: i64,
    bytes_sent: i64,
    viewers: i64,
}

impl MinuteTotals {
    /// Suma una medición. Si ya empezó otro minuto, devuelve el anterior resumido (para guardarlo)
    /// y arranca a juntar el nuevo.
    pub fn add(
        &mut self,
        now: i64,
        sample: &Sample,
        bytes: i64,
        viewers: usize,
    ) -> Option<MinuteRow> {
        let minute = now - now.rem_euclid(60);
        let finished = (minute != self.minute).then(|| self.finish()).flatten();
        if minute != self.minute {
            *self = MinuteTotals {
                minute,
                ..Default::default()
            };
        }
        // La CPU de Kinora y ffmpeg viene en "% de un núcleo": se pasa a % de toda la CPU,
        // así se compara con la del equipo en el mismo gráfico.
        let cores = sample.cpu_cores.max(1) as f64;
        self.count += 1;
        self.cpu_kinora += sample.cpu_kinora as f64 / cores;
        self.cpu_ffmpeg += sample.cpu_ffmpeg.unwrap_or(0.0) as f64 / cores;
        self.cpu_system += sample.cpu_system as f64;
        self.memory_kinora = self.memory_kinora.max(sample.memory_kinora);
        self.memory_ffmpeg = self.memory_ffmpeg.max(sample.memory_ffmpeg.unwrap_or(0));
        self.memory_used = self.memory_used.max(sample.memory_used);
        self.memory_total = sample.memory_total;
        self.bytes_sent += bytes;
        self.viewers = self.viewers.max(viewers as i64);
        finished
    }

    fn finish(&self) -> Option<MinuteRow> {
        if self.count == 0 {
            return None;
        }
        let n = self.count as f64;
        Some(MinuteRow {
            minute: self.minute,
            cpu_kinora: self.cpu_kinora / n,
            cpu_ffmpeg: self.cpu_ffmpeg / n,
            cpu_system: self.cpu_system / n,
            memory_kinora: self.memory_kinora,
            memory_ffmpeg: self.memory_ffmpeg,
            memory_used: self.memory_used,
            memory_total: self.memory_total,
            bytes_sent: self.bytes_sent,
            viewers: self.viewers,
        })
    }
}

/// Guarda el minuto y borra lo que tenga más de 30 días.
pub async fn save(db: &SqlitePool, row: &MinuteRow) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO server_samples \
         (minute, cpu_kinora, cpu_ffmpeg, cpu_system, memory_kinora, memory_ffmpeg, memory_used, \
          memory_total, bytes_sent, viewers) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(row.minute)
    .bind(row.cpu_kinora)
    .bind(row.cpu_ffmpeg)
    .bind(row.cpu_system)
    .bind(row.memory_kinora)
    .bind(row.memory_ffmpeg)
    .bind(row.memory_used)
    .bind(row.memory_total)
    .bind(row.bytes_sent)
    .bind(row.viewers)
    .execute(db)
    .await?;
    sqlx::query("DELETE FROM server_samples WHERE minute < ?")
        .bind(row.minute - KEEP_SECS)
        .execute(db)
        .await?;
    Ok(())
}

/// Qué período mostrar.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum HistoryRange {
    Day,
    Week,
    Month,
}

impl HistoryRange {
    /// Segundos que abarca y de cuántos segundos es cada punto: así cada gráfico tiene unos
    /// 300 puntos, sea un día o un mes.
    fn span_and_step(self) -> (i64, i64) {
        match self {
            HistoryRange::Day => (24 * 3600, 5 * 60),
            HistoryRange::Week => (7 * 24 * 3600, 30 * 60),
            HistoryRange::Month => (30 * 24 * 3600, 2 * 3600),
        }
    }
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct ServerHistory {
    /// Desde y hasta (segundos desde 1970), alineados a `step`.
    pub from: i64,
    pub to: i64,
    /// Segundos por punto.
    pub step: i64,
    /// Solo los períodos con mediciones: los huecos son ratos con el servidor apagado.
    pub points: Vec<HistoryPoint>,
}

#[derive(Serialize, sqlx::FromRow, TS)]
#[ts(export)]
pub struct HistoryPoint {
    /// Inicio del período (segundos desde 1970).
    pub t: i64,
    /// Promedios en % de toda la CPU (0–100).
    pub cpu_kinora: f64,
    pub cpu_ffmpeg: f64,
    pub cpu_system: f64,
    /// Memoria del equipo en uso, 0..1 (el máximo del período).
    pub memory_share: f64,
    /// Memoria de Kinora más la de ffmpeg si estaba convirtiendo, en bytes (el máximo del período).
    pub memory_kinora: i64,
    /// Velocidad media de envío en el período, en megabits por segundo.
    pub send_mbps: f64,
    /// Máximo de personas mirando a la vez.
    pub viewers: i64,
}

pub async fn load(db: &SqlitePool, range: HistoryRange, now: i64) -> sqlx::Result<ServerHistory> {
    let (span, step) = range.span_and_step();
    let to = now - now.rem_euclid(step) + step;
    let from = to - span;
    // La velocidad se promedia sobre los minutos medidos, no sobre todo el período: un rato
    // apagado no la baja.
    let points = sqlx::query_as(
        "SELECT (minute / ?1) * ?1 AS t, \
                AVG(cpu_kinora) AS cpu_kinora, AVG(cpu_ffmpeg) AS cpu_ffmpeg, \
                AVG(cpu_system) AS cpu_system, \
                MAX(CAST(memory_used AS REAL) / MAX(memory_total, 1)) AS memory_share, \
                MAX(memory_kinora + memory_ffmpeg) AS memory_kinora, \
                SUM(bytes_sent) * 8.0 / (COUNT(*) * 60) / 1000000.0 AS send_mbps, \
                MAX(viewers) AS viewers \
         FROM server_samples WHERE minute >= ?2 GROUP BY t ORDER BY t",
    )
    .bind(step)
    .bind(from)
    .fetch_all(db)
    .await?;
    Ok(ServerHistory {
        from,
        to,
        step,
        points,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connect_in_memory;

    #[tokio::test]
    async fn averages_each_minute_and_groups_by_range() {
        let sample = |cpu: f32| Sample {
            cpu_kinora: 200.0,
            cpu_system: cpu,
            cpu_cores: 10,
            cpu_ffmpeg: None,
            memory_kinora: 20_000_000,
            memory_used: 50,
            memory_total: 100,
            ..Default::default()
        };
        // Dos mediciones en el minuto 0 y una en el minuto 1: al llegar la del 1, sale el 0.
        let mut totals = MinuteTotals::default();
        assert!(totals.add(10, &sample(20.0), 1000, 1).is_none());
        assert!(totals.add(40, &sample(40.0), 500, 2).is_none());
        let row = totals
            .add(70, &sample(90.0), 0, 0)
            .expect("terminó el minuto 0");
        assert_eq!(row.minute, 0);
        assert!((row.cpu_system - 30.0).abs() < 1e-9);
        assert!(
            (row.cpu_kinora - 20.0).abs() < 1e-9,
            "200 % de un núcleo de 10 = 20 %"
        );
        assert_eq!((row.bytes_sent, row.viewers), (1500, 2));

        let db = connect_in_memory().await;
        save(&db, &row).await.unwrap();
        let history = load(&db, HistoryRange::Day, 100).await.unwrap();
        assert_eq!(history.step, 300);
        assert_eq!(history.points.len(), 1);
        let point = &history.points[0];
        assert!((point.memory_share - 0.5).abs() < 1e-9);
        assert_eq!(point.memory_kinora, 20_000_000);
        assert!((point.send_mbps - 1500.0 * 8.0 / 60.0 / 1e6).abs() < 1e-12);
    }
}
