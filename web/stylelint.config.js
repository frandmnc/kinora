// El CSS del Player tiene que andar en las TV: la Samsung de 2019 (Tizen 5.0, Chromium 63) y la
// VIDAA (un Chromium entre 74 y 79). Se controla con la más vieja.
// Se controla al compilar, para no enterarse en la TV. El Admin se usa desde la compu y no entra.
export default {
  plugins: ['stylelint-no-unsupported-browser-features'],
  rules: {
    'plugin/no-unsupported-browser-features': [
      true,
      {
        browsers: ['chrome 63'],
        severity: 'error',
        // Soporte parcial que no afecta: `overflow: clip`, el color de la barra de desplazamiento
        // y `mask-composite` sin prefijo (va junto al de -webkit-). Ojo: no detecta `aspect-ratio`.
        ignore: ['css-overflow', 'css-scrollbar', 'css-masks'],
      },
    ],
  },
}
