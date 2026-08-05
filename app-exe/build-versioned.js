const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const appExeDir = __dirname;
const rootDir = path.resolve(appExeDir, '..');

const tauriConfAppExe = path.join(appExeDir, 'src-tauri', 'tauri.conf.json');
const cargoTomlAppExe = path.join(appExeDir, 'src-tauri', 'Cargo.toml');
const pkgJsonAppExe = path.join(appExeDir, 'package.json');

const tauriConfRoot = path.join(rootDir, 'src-tauri', 'tauri.conf.json');
const cargoTomlRoot = path.join(rootDir, 'src-tauri', 'Cargo.toml');
const pkgJsonRoot = path.join(rootDir, 'package.json');

function bumpPatchVersion(versionStr) {
    const parts = String(versionStr || '1.0.0').split('.').map(n => parseInt(n, 10) || 0);
    while (parts.length < 3) parts.push(0);
    parts[2] += 1;
    return parts.join('.');
}

let currentVersion = '1.0.0';
if (fs.existsSync(tauriConfAppExe)) {
    try {
        const conf = JSON.parse(fs.readFileSync(tauriConfAppExe, 'utf8'));
        if (conf.version) currentVersion = conf.version;
    } catch (_) {}
}

const args = process.argv.slice(2);
let newVersion = '';
if (args[0] && /^\d+\.\d+\.\d+$/.test(args[0])) {
    newVersion = args[0];
} else {
    newVersion = bumpPatchVersion(currentVersion);
}

console.log(`\n================================================`);
console.log(`  BUILD PHIÊN BẢN MỚI: v${currentVersion} -> v${newVersion}`);
console.log(`================================================\n`);

function updateJsonVersion(filePath, version) {
    if (!fs.existsSync(filePath)) return;
    try {
        const content = fs.readFileSync(filePath, 'utf8');
        const json = JSON.parse(content);
        json.version = version;
        fs.writeFileSync(filePath, JSON.stringify(json, null, 2), 'utf8');
    } catch (err) {
        console.error(`Không thể cập nhật version cho ${filePath}:`, err.message);
    }
}

function updateCargoVersion(filePath, version) {
    if (!fs.existsSync(filePath)) return;
    try {
        let content = fs.readFileSync(filePath, 'utf8');
        content = content.replace(/^version\s*=\s*"[^"]+"/m, `version = "${version}"`);
        fs.writeFileSync(filePath, content, 'utf8');
    } catch (err) {
        console.error(`Không thể cập nhật version cho ${filePath}:`, err.message);
    }
}

updateJsonVersion(tauriConfAppExe, newVersion);
updateCargoVersion(cargoTomlAppExe, newVersion);
updateJsonVersion(pkgJsonAppExe, newVersion);

updateJsonVersion(tauriConfRoot, newVersion);
updateCargoVersion(cargoTomlRoot, newVersion);
updateJsonVersion(pkgJsonRoot, newVersion);

console.log(`✅ Đã đồng bộ version v${newVersion} vào tất cả file cấu hình.`);

console.log(`\n🚀 Đang tiến hành build bản cài đặt (.exe / .msi)...`);
try {
    execSync('npx @tauri-apps/cli build', { cwd: appExeDir, stdio: 'inherit' });
} catch (err) {
    console.error(`\n❌ Build thất bại! Vui lòng kiểm tra lỗi ở trên.`);
    process.exit(1);
}

const nsisDir = path.join(appExeDir, 'src-tauri', 'target', 'release', 'bundle', 'nsis');
const distDir = path.join(appExeDir, 'dist');

if (!fs.existsSync(distDir)) {
    fs.mkdirSync(distDir, { recursive: true });
}

let savedFiles = [];

if (fs.existsSync(nsisDir)) {
    const files = fs.readdirSync(nsisDir).filter(f => f.endsWith('.exe'));
    files.forEach(file => {
        const oldPath = path.join(nsisDir, file);
        const newFileName = `HangHoa_POS_v${newVersion}_Setup.exe`;
        const newPath = path.join(distDir, newFileName);
        fs.copyFileSync(oldPath, newPath);
        savedFiles.push(newPath);
    });
}

console.log(`\n================================================`);
console.log(`  🎉 BUILD THÀNH CÔNG PHIÊN BẢN v${newVersion}!`);
console.log(`================================================\n`);

if (savedFiles.length > 0) {
    console.log(`Các file bản build mới đã được lưu riêng không đè tại thư mục dist:`);
    savedFiles.forEach(f => console.log(`  📦 ${f}`));
} else {
    console.log(`Lưu ý: Không tìm thấy file trong thư mục bundle.`);
}
