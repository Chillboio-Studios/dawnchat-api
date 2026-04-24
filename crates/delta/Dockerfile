FROM debian:12 AS debian

# Bundle Stage
FROM gcr.io/distroless/cc-debian12:nonroot
COPY --chmod=755 ci-bin/revolt-delta ./
COPY --from=debian /usr/bin/uname /usr/bin/uname

EXPOSE 14702
ENV ROCKET_ADDRESS=0.0.0.0
USER nonroot
CMD ["./revolt-delta"]
