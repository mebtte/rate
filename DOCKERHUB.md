# rate

rate is a speed test you can host on your own server. Use it from your phone or computer to measure download speed, upload speed, latency, and jitter between your device and the server. The test page shows a live speed chart and your IP address with its approximate location, when available.

## Run with Docker

```sh
docker run -p 8080:8080 mebtte/rate:latest
```

Open `http://localhost:8080/` and select **Start test**. If Docker runs on another machine, use that machine's address in place of `localhost`.

The image supports `linux/amd64` and `linux/arm64`.

## More information

Visit the project on GitHub for standalone downloads, deployment instructions, and license details. [rate on GitHub](https://github.com/mebtte/rate#readme)
