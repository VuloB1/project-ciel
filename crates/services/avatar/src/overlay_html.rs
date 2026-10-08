/// The whole OBS Browser Source page — plain HTML/CSS/JS, no framework, no
/// build step (this is NOT part of the Vite/React app; it's served directly
/// by `routes.rs`'s `GET /overlay`). Kept intentionally tiny: preload the 3
/// state images up front (same fix `useAvatarStateImage.ts` already applies
/// on the React side, for the same reason — swapping which *already-loaded*
/// image is visible is instant, whereas fetching one lazily on each state
/// change routinely lost the race against how briefly "talking" lasts) and
/// toggle visibility on each WebSocket message. Reconnects with backoff
/// since OBS keeps a Browser Source loaded indefinitely across app restarts.
pub const OVERLAY_HTML: &str = r##"<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <title>Avatar</title>
    <style>
      html, body {
        margin: 0;
        padding: 0;
        width: 100%;
        height: 100%;
        background: transparent;
        overflow: hidden;
      }
      #stage {
        position: relative;
        width: 100vw;
        height: 100vh;
      }
      #stage img {
        position: absolute;
        inset: 0;
        width: 100%;
        height: 100%;
        object-fit: contain;
        opacity: 0;
        transition: opacity 80ms linear;
      }
      #stage img.visible {
        opacity: 1;
      }
    </style>
  </head>
  <body>
    <div id="stage">
      <img id="img-idle" alt="" />
      <img id="img-thinking" alt="" />
      <img id="img-talking" alt="" />
    </div>
    <script>
      (function () {
        var profileId = new URLSearchParams(location.search).get("profileId");
        var images = {
          idle: document.getElementById("img-idle"),
          thinking: document.getElementById("img-thinking"),
          talking: document.getElementById("img-talking"),
        };

        if (!profileId) {
          document.title = "Avatar — falta profileId en la URL";
          return;
        }

        Object.keys(images).forEach(function (state) {
          images[state].src = "/avatar/" + encodeURIComponent(profileId) + "/" + state;
        });

        function show(state) {
          Object.keys(images).forEach(function (key) {
            images[key].classList.toggle("visible", key === state);
          });
        }
        show("idle");

        function connect() {
          var proto = location.protocol === "https:" ? "wss:" : "ws:";
          var socket = new WebSocket(proto + "//" + location.host + "/ws?profileId=" + encodeURIComponent(profileId));

          socket.onmessage = function (event) {
            try {
              var data = JSON.parse(event.data);
              if (data && typeof data.state === "string") show(data.state);
            } catch (err) {
              // Ignore malformed frames — never worth tearing the socket down for.
            }
          };

          socket.onclose = function () {
            setTimeout(connect, 2000);
          };
          socket.onerror = function () {
            socket.close();
          };
        }
        connect();
      })();
    </script>
  </body>
</html>
"##;
