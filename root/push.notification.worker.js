self.addEventListener("push",event=>{
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
    tag: "tag",
    actions: [{
      action:url,//この辺のoptionが必須っぽい
      title:"open_app",
    }]
  };
  event.waitUntil(self.registration.showNotification(title, options));
});

self.addEventListener("notificationclick", event =>{
  event.notification.close();
  event.waitUntil(clients.openWindow(event.notification.data));
});