function GetCookies() {
    var result = new Array();

    var allcookies = document.cookie;
    if (allcookies != '') {
        var cookies = allcookies.split('; ');

        for (var i = 0; i < cookies.length; i++) {
            var cookie = cookies[i].split('=');

            // クッキーの名前をキーとして 配列に追加する
            result[cookie[0]] = decodeURIComponent(cookie[1]);
        }
    }
    return result;
}

function post_attendance(type, then) {
    let params = new URL(window.location.href).searchParams;
    let attendance_id = params.get('attendance_id');

    let Cookies = GetCookies();
    let user_id = Cookies['user_id'];

    if (user_id == null) {
        alert('LINEから認証してください');
        return;
    }

    var data = {
        'user_id': user_id,
        'attendance_id': attendance_id,
        'request_type': type
    };
    var json = JSON.stringify(data);

    const XHR = new XMLHttpRequest();
    XHR.open('POST', 'register');
    XHR.setRequestHeader('content-type', 'application/json');
    XHR.addEventListener('load', then);
    XHR.send(json);
}

function signup() {
    let params = new URL(window.location.href).searchParams;
    let user_id = params.get('user_id');
    if (user_id) {
        document.cookie = 'user_id=${user_id}';
        console.log('✅ ログインしました user_id=${user_id}');
    }
}