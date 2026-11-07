# rate

rate is a speed test you can host on your own server. Open the test page on your phone or computer and select **Start test** to measure the connection from that device to your server.

## What you can see

- Download speed, upload speed, latency, and jitter, plus a live chart of your speed during the test.
- Your IP address and its approximate location, when available. Location data comes from [IPinfo](https://ipinfo.io/).

The page adapts to phones and computers, uses Simplified Chinese or English based on your browser language, and follows your system's light or dark mode. You can also protect the page with a password or give it a custom URL path.

## How to deploy

### Run with Docker

```sh
docker run -d --name rate --restart unless-stopped -p 8080:8080 mebtte/rate:latest
```

Open `http://localhost:8080/` on the server. To run a test from a phone or another computer, open `http://<server-address>:8080/` on that device.

### Download and run

Download the file for your Linux, macOS, or Windows device from [GitHub Releases](https://github.com/mebtte/rate/releases).

On Linux or macOS, open a terminal in your download directory, make the file executable, and run it. This example uses the Linux x64 file. Substitute the name of the file you downloaded.

```sh
chmod +x rate-linux-amd64
./rate-linux-amd64
```

On Windows, open PowerShell in your download directory and run the downloaded file, for example `.\rate-windows-amd64.exe`.

Once rate is running, open `http://localhost:8080/`. To test from another device, add `--host 0.0.0.0` when starting rate, then open `http://<server-address>:8080/` on that device.

## License

rate is licensed under the GNU General Public License version 3 only (GPL-3.0-only). [License text](LICENSE)

The bundled IPinfo Lite database is licensed under Creative Commons Attribution-ShareAlike 4.0 International (CC BY-SA 4.0). [Third-party notices](THIRD_PARTY_NOTICES.md)
