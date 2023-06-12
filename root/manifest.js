var setManifestURL = function() {
    let params = new URL(window.location.href).searchParams;
    let user_id = params.get('user_id');
    var host = window.location.host;
    var myUrl = `https://${host}/`;
    var startUrl =  user_id ? myUrl + 'index' + "?user_id=" + user_id : myUrl + 'index';

    var manifest = {
      "name": "ブリッジ出欠",
      "short_name": "ブリッジ出欠",
      "description": "便利な出欠管理アプリだと思う",
      "theme_color": "#2196f3",
      "background_color": "#2196f3",
      "display": "standalone",
      "start_url": startUrl,
      "icons": [
        {
          "src": myUrl+"manifest/icon-192x192.png",
          "sizes": "192x192",
          "type": "image/png"
        },
        {
            "src": myUrl+"manifest/icon-256x256.png",
            "sizes": "256x256",
            "type": "image/png"
          },
        {
          "src": myUrl+"manifest/icon-384x384.png",
          "sizes": "384x384",
          "type": "image/png"
        },
        {
          "src": myUrl+"manifest/icon-512x512.png",
          "sizes": "512x512",
          "type": "image/png"
        }
      ]
    }

    const stringManifest = JSON.stringify(manifest);
    const blob = new Blob([stringManifest], {type: 'application/json'});
    const manifestURL = URL.createObjectURL(blob);
    document.querySelector('#my-manifest').setAttribute('href', manifestURL);
}
