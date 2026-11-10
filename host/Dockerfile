# XC Circuit Host: one image, two modes (XC_HOST_MODE=full or relay).
# Build from the repository's root:  docker build -f host/Dockerfile -t xc-circuit-host .
FROM node:24-alpine

# Git keeps the projects in full mode (git http-backend).
RUN apk add --no-cache git git-daemon tini

WORKDIR /app
COPY host/server.js ./server.js

# Not root; the data folder is the volume's, owned by this user.
RUN mkdir /data && chown node:node /data
USER node

ENV XC_HOST_MODE=full \
    XC_HOST_PORT=7421 \
    XC_HOST_DATA=/data \
    NODE_NO_WARNINGS=1

EXPOSE 7421
HEALTHCHECK --interval=30s --timeout=5s --retries=3 CMD wget -qO- http://127.0.0.1:${XC_HOST_PORT}/status || exit 1

ENTRYPOINT ["/sbin/tini", "--"]
CMD ["node", "server.js"]
