# rate

rate is a speed test you can host on your own server. Open the test page on your phone or computer and select **Start test** to measure the connection from that device to your server.

## What you can see

- Download speed, upload speed, latency, and jitter, plus a live chart of your speed during the test.
- Your IP address and its approximate location, when available. Location data comes from [IPinfo](https://ipinfo.io/).

The page adapts to phones and computers, uses Simplified Chinese or English based on your browser language, and follows your system's light or dark mode. You can also protect the page with a password or give it a custom URL path.

## How to deploy

### Download and run

Download the release for your operating system and CPU. [GitHub Releases](https://github.com/mebtte/rate/releases)

On Linux or macOS, run these commands in your download directory using the name of the file you downloaded.

```sh
chmod +x rate-linux-amd64
./rate-linux-amd64
```

On Windows, run the downloaded `.exe` in PowerShell, for example `.\rate-windows-amd64.exe`.

Open `http://localhost:8080/` in your browser.

### Run with Docker

```sh
docker run -p 8080:8080 mebtte/rate:latest
```

Open `http://localhost:8080/`, or use your server's address in place of `localhost`.

## License

rate is licensed under the GNU General Public License version 3 only (GPL-3.0-only). [License text](LICENSE)

The bundled IPinfo Lite database is licensed under Creative Commons Attribution-ShareAlike 4.0 International (CC BY-SA 4.0). [Third-party notices](THIRD_PARTY_NOTICES.md)
