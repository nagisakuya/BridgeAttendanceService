function receivePushNotification(event) {
  const json = event.data?.json() ?? {};
  const title = json.title || "TITLE";
  const message = json.message || "MESSAGE";
  const attendance_id = json.attendance_id;
  const user_id = json.user_id;

  var url = '';
  if (attendance_id){
    url = `result?attendance_id=${attendance_id}&user_id=${user_id}`;
  }else{
    url = `index`;
  }
  

  const options = {
    data: url,
    body: message,
  };
  event.waitUntil(this.registration.showNotification(title, options));
}
this.addEventListener("push", receivePushNotification);

function openPushNotification(event) {
  event.notification.close();
  event.waitUntil(clients.openWindow(event.notification.data));
}
this.addEventListener("notificationclick", openPushNotification);