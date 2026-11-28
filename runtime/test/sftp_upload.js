// The app's SFTP upload (crates/circuit-runner/src/hosting/sftp.rs) against
// a small stand-in SFTP server: starts it and runs the Rust test
// hosting::tests::real_sftp_upload, which logs in, sends a bot twice, checks
// that only changed files go again and that other files stay, and that a
// server whose key changed is refused. Skipped without ssh2.
// Usage: XC_TEST_DEPS=<dir with node_modules/ssh2> node runtime/test/sftp_upload.js
"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
const Module = require("module");
const { spawn } = require("child_process");

const root = path.join(__dirname, "..", "..");
const deps = process.env.XC_TEST_DEPS ? path.resolve(process.env.XC_TEST_DEPS) : null;
let ssh2 = null;
try {
  ssh2 = deps && Module.createRequire(path.join(deps, "package.json"))("ssh2");
} catch {}
if (!ssh2) {
  console.log("sftp upload: skipped (no ssh2 in XC_TEST_DEPS)");
  process.exit(0);
}
const { STATUS_CODE, OPEN_MODE } = ssh2.utils.sftp;

/// ssh2 makes an ed25519 key it can't read back itself about once in 200
/// (1.17.0): such a one is made again.
function hostKey() {
  for (;;) {
    const key = ssh2.utils.generateKeyPairSync("ed25519").private;
    if (!(ssh2.utils.parseKey(key) instanceof Error)) return key;
  }
}

// What the server serves: a file of its own that an upload must leave alone.
const home = fs.mkdtempSync(path.join(os.tmpdir(), "xc-sftp-home-"));
fs.mkdirSync(path.join(home, "data"));
fs.writeFileSync(path.join(home, "data", "keep.json"), "{}");
const at = (p) => path.join(home, p.replace(/^\/+/, ""));
const attrs = (st) => ({ mode: st.mode, uid: 0, gid: 0, size: st.size, atime: Math.floor(st.atimeMs / 1000), mtime: Math.floor(st.mtimeMs / 1000) });

// User "bot", password "test-password"; only what the upload uses. A rename
// onto a file that is there fails, as on most servers (SFTP 3).
const server = new ssh2.Server({ hostKeys: [hostKey()] }, (client) => {
  client.on("error", () => {});
  client.on("authentication", (ctx) => (ctx.method === "password" && ctx.username === "bot" && ctx.password === "test-password" ? ctx.accept() : ctx.reject(["password"])));
  client.on("ready", () =>
    client.on("session", (accept) =>
      accept().on("sftp", (acceptSftp) => {
        const sftp = acceptSftp();
        const files = new Map();
        let next = 0;
        const ok = (id) => sftp.status(id, STATUS_CODE.OK);
        const failed = (id) => sftp.status(id, STATUS_CODE.FAILURE);
        const missing = (id) => sftp.status(id, STATUS_CODE.NO_SUCH_FILE);
        const fd = (handle) => files.get(handle.readUInt32BE(0));
        sftp.on("REALPATH", (id, p) => sftp.name(id, [{ filename: path.posix.resolve("/", p), longname: "", attrs: {} }]));
        for (const ev of ["STAT", "LSTAT"]) {
          sftp.on(ev, (id, p) => {
            try {
              sftp.attrs(id, attrs(fs.statSync(at(p))));
            } catch {
              missing(id);
            }
          });
        }
        sftp.on("MKDIR", (id, p) => {
          try {
            fs.mkdirSync(at(p));
            ok(id);
          } catch {
            failed(id);
          }
        });
        sftp.on("OPEN", (id, p, flags) => {
          try {
            const handle = Buffer.alloc(4);
            handle.writeUInt32BE(next);
            files.set(next++, fs.openSync(at(p), flags & OPEN_MODE.WRITE ? "w" : "r"));
            sftp.handle(id, handle);
          } catch {
            failed(id);
          }
        });
        sftp.on("WRITE", (id, handle, offset, data) => {
          try {
            fs.writeSync(fd(handle), data, 0, data.length, offset);
            ok(id);
          } catch {
            failed(id);
          }
        });
        sftp.on("FSTAT", (id, handle) => {
          try {
            sftp.attrs(id, attrs(fs.fstatSync(fd(handle))));
          } catch {
            failed(id);
          }
        });
        sftp.on("CLOSE", (id, handle) => {
          try {
            fs.closeSync(fd(handle));
          } catch {}
          files.delete(handle.readUInt32BE(0));
          ok(id);
        });
        sftp.on("RENAME", (id, from, to) => {
          if (fs.existsSync(at(to))) return failed(id);
          try {
            fs.renameSync(at(from), at(to));
            ok(id);
          } catch {
            failed(id);
          }
        });
        sftp.on("REMOVE", (id, p) => {
          try {
            fs.unlinkSync(at(p));
            ok(id);
          } catch {
            missing(id);
          }
        });
        sftp.on("SETSTAT", ok);
        sftp.on("FSETSTAT", ok);
      }),
    ),
  );
});

server.listen(0, "127.0.0.1", () => {
  const { port } = server.address();
  const test = spawn("cargo", ["test", "-p", "circuit-runner", "--lib", "hosting::tests::real_sftp_upload", "--", "--ignored", "--exact"], {
    cwd: root,
    stdio: "inherit",
    env: { ...process.env, XC_TEST_SFTP: `127.0.0.1:${port}`, XC_TEST_SFTP_HOME: home },
  });
  test.on("exit", (code) => {
    server.close();
    fs.rmSync(home, { recursive: true, force: true });
    console.log(code === 0 ? "sftp upload: ok" : "sftp upload: FAILED");
    process.exit(code ?? 1);
  });
});
