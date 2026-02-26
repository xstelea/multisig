# Build stage
FROM node:22-alpine AS builder

RUN corepack enable && corepack prepare pnpm@10.25.0 --activate

WORKDIR /app

# Copy workspace root files
COPY package.json pnpm-workspace.yaml pnpm-lock.yaml ./
COPY multisig-app/ multisig-app/

RUN pnpm install --frozen-lockfile

ARG VITE_PUBLIC_NETWORK_ID
ARG VITE_PUBLIC_DAPP_DEFINITION_ADDRESS
ARG VITE_ORCHESTRATOR_URL

WORKDIR /app/multisig-app
RUN pnpm run build

# Runtime stage
FROM node:22-alpine

WORKDIR /app

COPY --from=builder /app/multisig-app/.output .output/

EXPOSE 3000

CMD ["node", ".output/server/index.mjs"]
