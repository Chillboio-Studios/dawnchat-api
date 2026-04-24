# Bundle Stage
FROM gcr.io/distroless/cc-debian12:nonroot
COPY ci-bin/revolt-gifbox ./

EXPOSE 14706
USER nonroot
CMD ["./revolt-gifbox"]
