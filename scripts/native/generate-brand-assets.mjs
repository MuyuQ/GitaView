import { readFile, writeFile, copyFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import sharp from 'sharp';

const root = fileURLToPath(new URL('../../', import.meta.url));
const source = path.join(root, 'assets/brand/gitaview.svg');
const destination = path.join(root, 'apps/windows/GitaView.Native/Assets');
const svg = await readFile(source);

// All native sizes come from the shared vector, including the small ICO frames.
const assets = [
  ['GitaView.png', 256, 256, 256],
  ['Square150x150Logo.scale-200.png', 300, 300, 300],
  ['Square44x44Logo.scale-200.png', 88, 88, 88],
  ['Square44x44Logo.targetsize-24_altform-unplated.png', 24, 24, 24],
  ['Square44x44Logo.targetsize-48_altform-lightunplated.png', 48, 48, 48],
  ['LockScreenLogo.scale-200.png', 48, 48, 48],
  ['StoreLogo.png', 50, 50, 50],
  ['Wide310x150Logo.scale-200.png', 620, 300, 240],
  ['SplashScreen.scale-200.png', 1240, 600, 192],
];

async function render(size) {
  return sharp(svg, { density: 384 }).resize(size, size).png().toBuffer();
}

for (const [name, width, height, markSize] of assets) {
  const icon = await render(markSize);
  const image = width === markSize && height === markSize
    ? icon
    : await sharp({ create: { width, height, channels: 4, background: '#00000000' } })
      .composite([{ input: icon, gravity: 'centre' }]).png().toBuffer();
  await writeFile(path.join(destination, name), image);
}
await copyFile(source, path.join(destination, 'GitaView.svg'));

const sizes = [16, 20, 24, 32, 40, 48, 64, 128, 256];
const frames = await Promise.all(sizes.map(render));
const directory = Buffer.alloc(6 + sizes.length * 16);
directory.writeUInt16LE(1, 2);
directory.writeUInt16LE(sizes.length, 4);
let offset = directory.length;
for (let i = 0; i < sizes.length; i++) {
  const entry = 6 + i * 16;
  directory[entry] = sizes[i] === 256 ? 0 : sizes[i];
  directory[entry + 1] = directory[entry];
  directory.writeUInt16LE(1, entry + 4);
  directory.writeUInt16LE(32, entry + 6);
  directory.writeUInt32LE(frames[i].length, entry + 8);
  directory.writeUInt32LE(offset, entry + 12);
  offset += frames[i].length;
}
await writeFile(path.join(destination, 'AppIcon.ico'), Buffer.concat([directory, ...frames]));
console.log(`Generated ${assets.length} PNGs, SVG, and ${sizes.length}-frame ICO from assets/brand/gitaview.svg`);
