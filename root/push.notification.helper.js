/**
 * プッシュ通知が使えるかを確認
 */
function notification_isSupported() {
    const isSupported = "serviceWorker" in navigator && "PushManager" in window;
    if (isSupported) {
        console.log("✅（１）プッシュ通知機能が使える");
    } else {
        console.log("❌（１）プッシュ通知機能が使えない");
    }
    return isSupported;
}

/**
 * ブラウザ上のプッシュ通知の許可を得る
 */
function notification_askForPermission() {
    // request user grant to show notification
    Notification.requestPermission(function (result) {
        return result;
    }).then(
        (success) => {
            console.log("✅ （２）プッシュ通知の権限がある");
        },
        (fail) => {
            console.log("❌ （２）プッシュ通知の権限がない");
        });
}



/**
 * プッシュ通知を処理する「サービスワーカ」を立ち上げる。
 */
function notification_startServiceWorker() {
    return navigator.serviceWorker.register("push.notification.worker.js").then(
        (registration) => {
            console.log("✅ （３）サービスワーカを立ち上げた");
        },
        (error) => {
            console.log("❌ （３）サービスワーカを立ち上げることができなかった。プッシュ通知を授受できない");
            console.log(error);
        });
}


/**
 * 通知表示機能を試す（通知を送信しない）
 * 条件：notification_startServiceWorkerが成功した
 */
function notification_test_showNotification() {

    const title = "通知のタイトル";
    const options = {
        body: "通知の中身",
    };

    navigator.serviceWorker.ready.then((serviceWorker) => {
        serviceWorker.showNotification(title, options).then(
            (success) => {
                console.log("✅ （４）通知を表示できた（つもり）");
            },
            (error) => {
                console.log("❌ （４）通知を表示できません");
                console.log(error);
            });
    });
}

/**
 * ブラウザのプッシュサービスに登録する
 * 条件：notification_startServiceWorkerが成功した
 */
function notification_subscribe() {
    const pushServerPublicKey = 'BERr1Xm5hN40diIp-Pk1mg7EKOkBAaGTurr0XWq-lPYo5_y-TaXwPIxi5R7GjaJblooHDbLttJ7HxqEXXOyA9ds';
    return navigator.serviceWorker.ready.then(
        (serviceWorker) => {
            // subscribe and return the subscription
            return serviceWorker.pushManager
                .subscribe({
                    userVisibleOnly: true,
                    applicationServerKey: pushServerPublicKey
                }).then(
                    (subscription) => {
                        // TODO: send subscription.endpoint to server
                        post_subscription(subscription)

                        console.log("✅ （５）登録が成功した。次の情報を使ってプッシュ通知を送れる")
                        console.log(subscription.toJSON())
                        //_showWebPushCommand(subscription.toJSON());
                        return subscription;
                    },
                    (error) => {
                        console.log("❌ （５）プッシュサービスに登録できなかった");
                        console.log(error);
                    });
        }
    );
}

// プッシュ通知の登録情報をプッシュサーバに送信する
function post_subscription(subscription) {
    var subscriptionInfo = subscription.toJSON();
    var data = {
        'user_id': GetCookies()['user_id'],
        'key': subscriptionInfo.keys.p256dh,
        'auth': subscriptionInfo.keys.auth,
        'endpoint': subscriptionInfo.endpoint,
    };
    var json = JSON.stringify(data);

    const XHR = new XMLHttpRequest();
    XHR.open('POST', 'subscribe');
    XHR.setRequestHeader('content-type', 'application/json');
    XHR.send(json);
}

function notification_unsubscribe() {
    navigator.serviceWorker.ready.then(function (reg) {
        reg.pushManager.getSubscription().then(function (subscription) {
            subscription.unsubscribe().then(function (successful) {
                console.log("✅ 登録解除した")
                console.log(subscription.toJSON())
            }).catch(function (e) {
                console.log("❌ プッシュサービスを登録解除できなかった");
                console.log(error);
            })
        })
    });
}