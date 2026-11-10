# XC Circuit Host

The container people run themselves for [XC Circuit](https://github.com/XCraftTM/XC-Circuit): projects, secrets and live sessions on a server of your own, or a relay that only passes live sessions on.

This branch holds only the Host, so its image can be built before the app that uses it is released. The Host's home is the `host/` folder of the `main` branch; changes are made there and copied here.

- Image: `ghcr.io/xcrafttm/xc-circuit-host` (x64 and ARM), built by `.github/workflows/host-image.yml`
- Start it: `host/docker-compose.yml`
- Tests: `node host/test/host.js` (Node.js 24)

Settings: `XC_HOST_MODE` (`full`, the default, or `relay`), `XC_HOST_PORT` (7421), `XC_HOST_DATA` (`/data`, full mode needs it as a volume), `XC_HOST_TRUSTED_PROXY` (`private`, or the proxy's addresses).
