// El CSS del Player tiene que andar en el navegador de la TV con VIDAA: un Chromium entre 74 y 79.
// Se controla al compilar, para no enterarse en la TV. El Admin se usa desde la compu y no entra.
export default {
  plugins: ['stylelint-no-unsupported-browser-features'],
  rules: {
    'plugin/no-unsupported-browser-features': [
      true,
      {
        browsers: ['chrome 74'],
        severity: 'error',
        // Soporte parcial que no afecta: `overflow: clip`, el color de la barra de desplazamiento
        // y `mask-composite` sin prefijo (va junto al de -webkit-). Ojo: no detecta `aspect-ratio`.
        ignore: ['css-overflow', 'css-scrollbar', 'css-masks'],
      },
    ],
  },
}
