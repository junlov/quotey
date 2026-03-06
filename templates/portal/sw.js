const CACHE_NAME = 'quotey-portal-v2';
const STATIC_ASSETS = [
  '/portal',
  '/approvals',
  '/settings',
  '/manifest.webmanifest',
  '/portal/manifest.webmanifest',
];

self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(CACHE_NAME).then((cache) => cache.addAll(STATIC_ASSETS)).catch(() => Promise.resolve()),
  );
  self.skipWaiting();
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys().then((keys) =>
      Promise.all(keys.filter((key) => key !== CACHE_NAME).map((key) => caches.delete(key))),
    ).catch(() => Promise.resolve()),
  );
  self.clients.claim();
});

self.addEventListener('fetch', (event) => {
  if (event.request.method !== 'GET') return;

  const url = new URL(event.request.url);
  const isApprovalReadRoute = url.pathname.startsWith('/approvals');
  const isSensitiveQuoteRoute = url.pathname.startsWith('/quote/');
  const isApiRoute = url.pathname.startsWith('/api/');

  // Never cache token-bearing quote pages or API responses.
  if (isSensitiveQuoteRoute || isApiRoute) {
    event.respondWith(
      fetch(event.request).catch(
        () => new Response('Offline', { status: 503, statusText: 'Service Unavailable' }),
      ),
    );
    return;
  }

  // Approval reads should prefer fresh server state; fall back to cache if offline.
  if (isApprovalReadRoute) {
    event.respondWith(
      fetch(event.request)
        .then((response) => {
          if (response && response.status === 200) {
            const clone = response.clone();
            caches.open(CACHE_NAME).then((cache) => cache.put(event.request, clone)).catch(() => {});
          }
          return response;
        })
        .catch(
          () =>
            caches.match(event.request).then(
              (cached) =>
                cached || new Response('Offline', { status: 503, statusText: 'Service Unavailable' }),
            ),
        ),
    );
    return;
  }

  event.respondWith(
    caches.match(event.request).then((cached) => {
      if (cached) return cached;
      return fetch(event.request).then((response) => {
        if (response && response.status === 200) {
          const clone = response.clone();
          caches.open(CACHE_NAME).then((cache) => cache.put(event.request, clone)).catch(() => {});
        }
        return response;
      });
    }).catch(
      () =>
        new Response('Offline', { status: 503, statusText: 'Service Unavailable' }),
    ),
  );
});

function normalizeNotificationUrl(rawUrl) {
  if (typeof rawUrl !== 'string' || rawUrl.trim() === '') return '/approvals';
  try {
    const parsed = new URL(rawUrl, self.location.origin);
    if (parsed.origin !== self.location.origin) return '/approvals';
    return `${parsed.pathname}${parsed.search}${parsed.hash}`;
  } catch (_) {
    return '/approvals';
  }
}

self.addEventListener('push', (event) => {
  let payload = {
    title: 'Approval Request',
    body: 'A quote is waiting for your review.',
    url: '/approvals',
  };

  if (event.data) {
    try {
      payload = { ...payload, ...event.data.json() };
    } catch (_) {
      payload.body = event.data.text() || payload.body;
    }
  }

  const notificationUrl = normalizeNotificationUrl(payload.url);
  event.waitUntil(
    self.registration.showNotification(payload.title, {
      body: payload.body,
      icon: payload.icon || undefined,
      badge: payload.badge || undefined,
      data: { url: notificationUrl },
    }),
  );
});

self.addEventListener('notificationclick', (event) => {
  event.notification.close();
  const destination = normalizeNotificationUrl(event.notification.data?.url);
  event.waitUntil(clients.openWindow(destination));
});
