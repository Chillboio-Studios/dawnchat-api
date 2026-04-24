FROM debian:12 AS debian

# Bundle Stage
FROM gcr.io/distroless/cc-debian12:nonroot
COPY ci-bin/revolt-bonfire ./
COPY --from=debian /usr/bin/uname /usr/bin/uname

EXPOSE 14703
USER nonroot
CMD ["./revolt-bonfire"]
