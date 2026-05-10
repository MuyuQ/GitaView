const sharp = require('sharp');
const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32">
  <rect width="32" height="32" fill="#172033" rx="4"/>
  <text x="16" y="24" text-anchor="middle" fill="white" font-family="monospace" font-size="20" font-weight="bold">G</text>
</svg>`;
sharp(Buffer.from(svg))
  .toFile('src-tauri/icons/icon.png')
  .then(() => console.log('png created'));
