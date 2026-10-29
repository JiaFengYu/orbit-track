import { apiBase } from './config.js';

function endpoint(path) {
  if (apiBase === null) throw Error('The interactive demo is not available yet. Please check back soon.');
  const base = new URL(apiBase || location.origin);
  const local = ['localhost', '127.0.0.1', '[::1]'].includes(base.hostname);
  if ((base.protocol !== 'https:' && !(local && base.protocol === 'http:')) || base.username || base.password || base.search || base.hash || base.pathname !== '/') {
    throw Error('The interactive demo is temporarily unavailable.');
  }
  return new URL(path, base);
}

async function get(path, options = {}) {
  let response;
  try {
    response = await fetch(endpoint(path), { ...options, credentials: 'omit', signal: AbortSignal.timeout(90000) });
  } catch (error) {
    if (error.name === 'TimeoutError' || error instanceof TypeError) {
      throw Error('The orbit service could not be reached. Please try again in a moment.');
    }
    throw error;
  }
  if (!response.headers.get('content-type')?.includes('application/json')) {
    throw Error('The orbit service is temporarily unavailable. Please try again shortly.');
  }
  const data = await response.json();
  if (!response.ok) throw Error(data.error?.message ?? `The orbit service returned HTTP ${response.status}.`);
  return data;
}

export const api = {
  catalog: () => get('/v1/satellites'),
  predict: (id, query) => get(`/v1/satellites/${encodeURIComponent(id)}/track?${query}&format=explorer`),
  explore: request => get('/v1/explore', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(request) }),
};
