FROM debian:12 AS debian

# Bundle Stage
FROM gcr.io/distroless/cc-debian12:nonroot
COPY ci-bin/revolt-voice-ingress ./
COPY --from=debian /usr/bin/uname /usr/bin/uname

USER nonroot
CMD ["./revolt-voice-ingress"]