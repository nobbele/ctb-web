import { defineNuxtPlugin, useState } from '#app'

type NotificationType = 'success' | 'failure';

interface Notification {
  id: number,

  message: string,
  type: NotificationType ,

  active: boolean,
  visible: boolean,
}

export default defineNuxtPlugin(() => {
  const notifications = useState<Notification[]>('notifications', () => []);
  const notificationId = useState<number>('notificationId', () => 0);
  return {
    provide: {
      notifications,
      notify(message: string, type: NotificationType = "success") {
        const id = notificationId.value;
        notifications.value.push({
          id,
          message,
          type,
          active: true,
          visible: false,
        });
        setTimeout(() => {
          notifications.value.find(not => not.id == id)!.visible = true;
          setTimeout(() => {
            notifications.value.find(not => not.id == id)!.visible = false;
            setTimeout(() => {
              notifications.value.find(not => not.id == id)!.active = false;
            }, 650);
          }, 1500);
        }, 100);

        notificationId.value += 1;
      }
    }
  }
})