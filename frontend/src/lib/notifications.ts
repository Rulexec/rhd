export async function requestNotificationPermission(): Promise<void> {
  if ('Notification' in window && Notification.permission === 'default') {
    await Notification.requestPermission();
  }
}

export function showNotification(title: string, body: string, onClick?: () => void): void {
  if (!('Notification' in window)) {
    return;
  }

  if (Notification.permission !== 'granted') {
    return;
  }

  const notification = new Notification(title, {
    body,
    icon: '/favicon.ico',
  });

  if (onClick) {
    notification.onclick = () => {
      window.focus();
      onClick();
    };
  }
}
