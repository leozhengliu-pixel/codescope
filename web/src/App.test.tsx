import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { App } from './App';

function jsonResponse(body: unknown, ok = true, status = 200): Response {
  return {
    ok,
    status,
    json: async () => body,
  } as Response;
}

function deferredResponse() {
  let resolve!: (response: Response) => void;
  const promise = new Promise<Response>((innerResolve) => {
    resolve = innerResolve;
  });

  return { promise, resolve };
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  window.localStorage.clear();
  window.location.hash = '#/';
});

describe('App', () => {
  it('renders repository names after the list request succeeds', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
      jsonResponse([
        {
          id: 'repo-1',
          name: 'alpha-repo',
          default_branch: 'main',
          sync_state: 'ready',
        },
      ])
    );

    render(<App />);

    expect(await screen.findByText('Default branch: main')).toBeInTheDocument();
  });

  it('renders a dedicated search route with top-level navigation instead of falling back to the repository home shell', async () => {
    window.location.hash = '#/search';

    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
      jsonResponse([
        {
          id: 'repo-1',
          name: 'alpha-repo',
          default_branch: 'main',
          sync_state: 'ready',
        },
      ])
    );

    render(<App />);

    expect(screen.getByRole('link', { name: 'Search' })).toHaveAttribute('href', '#/search');
    expect(screen.getByText('Run API-backed code search across repositories from a dedicated route.')).toBeInTheDocument();
    expect(await screen.findByText('Enter a query to search indexed code.')).toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'Repositories' })).not.toBeInTheDocument();
  });

  it('renders a dedicated ask route, submits repo-scoped prompts with stored auth, and shows cited answers inline', async () => {
    window.location.hash = '#/ask?repo_id=repo-1';
    window.localStorage.setItem(
      'sourcebot-local-session',
      JSON.stringify({
        sessionId: 'session-local',
        sessionSecret: 'secret-local',
      })
    );

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/repos') {
        expect(init?.headers).toMatchObject({
          Authorization: 'Bearer session-local:secret-local',
        });
        return jsonResponse([
          {
            id: 'repo-1',
            name: 'alpha-repo',
            default_branch: 'main',
            sync_state: 'ready',
          },
          {
            id: 'repo-2',
            name: 'beta-repo',
            default_branch: 'main',
            sync_state: 'ready',
          },
        ]);
      }

      if (url === '/api/v1/ask/completions') {
        expect(init?.method).toBe('POST');
        expect(init?.headers).toMatchObject({
          Authorization: 'Bearer session-local:secret-local',
          'Content-Type': 'application/json',
        });
        expect(init?.body).toBe(
          JSON.stringify({
            prompt: 'Where is the router defined?',
            repo_scope: ['repo-1'],
          })
        );

        return jsonResponse({
          provider: 'test-provider',
          model: 'test-model',
          answer: 'The router is defined in src/lib.rs.',
          citations: [
            {
              repo_id: 'repo-1',
              path: 'src/lib.rs',
              revision: 'deadbeef',
              line_start: 10,
              line_end: 12,
            },
          ],
          rendered_citations: [
            {
              repo_id: 'repo-1',
              path: 'src/lib.rs',
              revision: 'deadbeef',
              line_start: 10,
              line_end: 12,
              display_label: 'src/lib.rs#L10-L12',
              pinned_location: 'src/lib.rs@deadbeef#L10-L12',
              line_fragment: 'L10-L12',
            },
          ],
          thread_id: 'thread-1',
          session_id: 'session-1',
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(screen.getByRole('link', { name: 'Ask' })).toHaveAttribute('href', '#/ask');
    expect(screen.getByText('Ask grounded questions across visible repositories from a dedicated route.')).toBeInTheDocument();
    expect(await screen.findByText('Choose a repository scope and submit a question to the ask completions API.')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Question'), { target: { value: 'Where is the router defined?' } });
    fireEvent.click(screen.getByRole('button', { name: 'Ask' }));

    expect(await screen.findByText('The router is defined in src/lib.rs.')).toBeInTheDocument();
    expect(screen.getByText('Current scope: alpha-repo')).toBeInTheDocument();
    expect(screen.getByText('Provider: test-provider')).toBeInTheDocument();
    expect(screen.getByText('Model: test-model')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'src/lib.rs#L10-L12' })).toHaveAttribute(
      'href',
      '#/repos/repo-1?path=src%2Flib.rs&revision=deadbeef'
    );
    expect(window.location.hash).toBe('#/ask?repo_id=repo-1&thread_id=thread-1');
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it('restores the ask thread from the hash route for follow-up asks', async () => {
    window.location.hash = '#/ask?repo_id=repo-1&thread_id=thread-9';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/repos') {
        return jsonResponse([
          {
            id: 'repo-1',
            name: 'alpha-repo',
            default_branch: 'main',
            sync_state: 'ready',
          },
        ]);
      }

      if (url === '/api/v1/ask/completions') {
        expect(init?.method).toBe('POST');
        expect(init?.body).toBe(
          JSON.stringify({
            prompt: 'What about follow-up context?',
            repo_scope: ['repo-1'],
            thread_id: 'thread-9',
          })
        );

        return jsonResponse({
          provider: 'test-provider',
          model: 'follow-up-model',
          answer: 'Follow-up answers stay on the restored thread.',
          citations: [],
          rendered_citations: [],
          thread_id: 'thread-9',
          session_id: 'session-9',
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Active thread: thread-9')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Question'), { target: { value: 'What about follow-up context?' } });
    fireEvent.click(screen.getByRole('button', { name: 'Ask' }));

    expect(await screen.findByText('Follow-up answers stay on the restored thread.')).toBeInTheDocument();
    expect(window.location.hash).toBe('#/ask?repo_id=repo-1&thread_id=thread-9');
  });

  it('clears a stale restored ask thread after a 404 follow-up and resets the hash to a fresh thread baseline', async () => {
    window.location.hash = '#/ask?repo_id=repo-1&thread_id=thread-stale';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/repos') {
        return jsonResponse([
          {
            id: 'repo-1',
            name: 'alpha-repo',
            default_branch: 'main',
            sync_state: 'ready',
          },
        ]);
      }

      if (url === '/api/v1/ask/completions') {
        expect(init?.body).toBe(
          JSON.stringify({
            prompt: 'Resume the stale thread',
            repo_scope: ['repo-1'],
            thread_id: 'thread-stale',
          })
        );
        return jsonResponse({ error: 'missing thread' }, false, 404);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Active thread: thread-stale')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Question'), { target: { value: 'Resume the stale thread' } });
    fireEvent.click(screen.getByRole('button', { name: 'Ask' }));

    expect(await screen.findByText(/The restored ask thread is no longer available for this repository scope\. Start a fresh thread\./)).toBeInTheDocument();
    expect(screen.queryByText('Active thread: thread-stale')).not.toBeInTheDocument();
    expect(window.location.hash).toBe('#/ask?repo_id=repo-1');
  });

  it('drops a restored ask thread when the hash route repo is not visible and falls back to a visible repo', async () => {
    window.location.hash = '#/ask?repo_id=repo-hidden&thread_id=thread-9';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input);

      if (url === '/api/v1/repos') {
        return jsonResponse([
          {
            id: 'repo-1',
            name: 'alpha-repo',
            default_branch: 'main',
            sync_state: 'ready',
          },
          {
            id: 'repo-2',
            name: 'beta-repo',
            default_branch: 'main',
            sync_state: 'ready',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Choose a repository scope and submit a question to the ask completions API.')).toBeInTheDocument();
    expect(screen.getByText('Current scope: alpha-repo')).toBeInTheDocument();
    expect(screen.queryByText('Active thread: thread-9')).not.toBeInTheDocument();
    expect(window.location.hash).toBe('#/ask?repo_id=repo-1');
  });

  it('clears the restored ask thread when the repository scope changes before a follow-up ask', async () => {
    window.location.hash = '#/ask?repo_id=repo-1&thread_id=thread-9';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/repos') {
        return jsonResponse([
          {
            id: 'repo-1',
            name: 'alpha-repo',
            default_branch: 'main',
            sync_state: 'ready',
          },
          {
            id: 'repo-2',
            name: 'beta-repo',
            default_branch: 'main',
            sync_state: 'ready',
          },
        ]);
      }

      if (url === '/api/v1/ask/completions') {
        expect(init?.method).toBe('POST');
        expect(init?.body).toBe(
          JSON.stringify({
            prompt: 'Ask beta follow-up',
            repo_scope: ['repo-2'],
          })
        );

        return jsonResponse({
          provider: 'test-provider',
          model: 'follow-up-model',
          answer: 'Changing repository scope starts a fresh ask thread.',
          citations: [],
          rendered_citations: [],
          thread_id: 'thread-22',
          session_id: 'session-22',
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Active thread: thread-9')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Repository scope'), { target: { value: 'repo-2' } });
    await waitFor(() => {
      expect(screen.queryByText('Active thread: thread-9')).not.toBeInTheDocument();
    });
    expect(window.location.hash).toBe('#/ask?repo_id=repo-2');

    fireEvent.change(screen.getByLabelText('Question'), { target: { value: 'Ask beta follow-up' } });
    fireEvent.click(screen.getByRole('button', { name: 'Ask' }));

    expect(await screen.findByText('Changing repository scope starts a fresh ask thread.')).toBeInTheDocument();
    expect(window.location.hash).toBe('#/ask?repo_id=repo-2&thread_id=thread-22');
  });

  it('clears stale ask answers when the hash route changes to a different repo scope', async () => {
    window.location.hash = '#/ask?repo_id=repo-1';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/repos') {
        return jsonResponse([
          {
            id: 'repo-1',
            name: 'alpha-repo',
            default_branch: 'main',
            sync_state: 'ready',
          },
          {
            id: 'repo-2',
            name: 'beta-repo',
            default_branch: 'main',
            sync_state: 'ready',
          },
        ]);
      }

      if (url === '/api/v1/ask/completions') {
        expect(init?.method).toBe('POST');
        return jsonResponse({
          provider: 'test-provider',
          model: 'test-model',
          answer: 'The first ask answer should disappear after route navigation.',
          citations: [],
          rendered_citations: [],
          thread_id: 'thread-1',
          session_id: 'session-1',
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Choose a repository scope and submit a question to the ask completions API.')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Question'), { target: { value: 'Where is the router defined?' } });
    fireEvent.click(screen.getByRole('button', { name: 'Ask' }));

    expect(await screen.findByText('The first ask answer should disappear after route navigation.')).toBeInTheDocument();
    expect(screen.getByText('Current scope: alpha-repo')).toBeInTheDocument();

    window.location.hash = '#/ask?repo_id=repo-2';
    window.dispatchEvent(new HashChangeEvent('hashchange'));

    await waitFor(() => {
      expect(screen.queryByText('The first ask answer should disappear after route navigation.')).not.toBeInTheDocument();
    });
    expect(screen.getByText('Current scope: beta-repo')).toBeInTheDocument();
    expect(screen.getByText('Choose a repository scope and submit a question to the ask completions API.')).toBeInTheDocument();
  });

  it('ignores stale ask responses that resolve after the route switches to a different repo scope', async () => {
    window.location.hash = '#/ask?repo_id=repo-1';

    const delayedAsk = deferredResponse();
    let askCallCount = 0;

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/repos') {
        return jsonResponse([
          {
            id: 'repo-1',
            name: 'alpha-repo',
            default_branch: 'main',
            sync_state: 'ready',
          },
          {
            id: 'repo-2',
            name: 'beta-repo',
            default_branch: 'main',
            sync_state: 'ready',
          },
        ]);
      }

      if (url === '/api/v1/ask/completions') {
        askCallCount += 1;

        if (askCallCount === 1) {
          expect(init?.body).toBe(
            JSON.stringify({
              prompt: 'Question for alpha',
              repo_scope: ['repo-1'],
            })
          );
          return delayedAsk.promise;
        }

        expect(init?.body).toBe(
          JSON.stringify({
            prompt: 'Question for beta',
            repo_scope: ['repo-2'],
          })
        );
        return jsonResponse({
          provider: 'test-provider',
          model: 'fresh-model',
          answer: 'Fresh beta answer should win.',
          citations: [],
          rendered_citations: [],
          thread_id: 'thread-2',
          session_id: 'session-2',
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Choose a repository scope and submit a question to the ask completions API.')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Question'), { target: { value: 'Question for alpha' } });
    fireEvent.click(screen.getByRole('button', { name: 'Ask' }));
    expect(screen.getByText('Loading answer…')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Repository scope'), { target: { value: 'repo-2' } });
    await waitFor(() => {
      expect(window.location.hash).toBe('#/ask?repo_id=repo-2');
    });

    fireEvent.change(screen.getByLabelText('Question'), { target: { value: 'Question for beta' } });
    fireEvent.click(screen.getByRole('button', { name: 'Ask' }));

    expect(await screen.findByText('Fresh beta answer should win.')).toBeInTheDocument();
    expect(window.location.hash).toBe('#/ask?repo_id=repo-2&thread_id=thread-2');

    delayedAsk.resolve(
      jsonResponse({
        provider: 'test-provider',
        model: 'stale-model',
        answer: 'Stale alpha answer must be ignored.',
        citations: [],
        rendered_citations: [],
        thread_id: 'thread-1',
        session_id: 'session-1',
      })
    );

    await waitFor(() => {
      expect(screen.queryByText('Stale alpha answer must be ignored.')).not.toBeInTheDocument();
      expect(window.location.hash).toBe('#/ask?repo_id=repo-2&thread_id=thread-2');
    });
  });

  it('invalidates an in-flight ask before a repo-scope change can let a stale response rewrite the route hash', async () => {
    window.location.hash = '#/ask?repo_id=repo-1';

    const delayedAsk = deferredResponse();

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/repos') {
        return jsonResponse([
          {
            id: 'repo-1',
            name: 'alpha-repo',
            default_branch: 'main',
            sync_state: 'ready',
          },
          {
            id: 'repo-2',
            name: 'beta-repo',
            default_branch: 'main',
            sync_state: 'ready',
          },
        ]);
      }

      if (url === '/api/v1/ask/completions') {
        expect(init?.body).toBe(
          JSON.stringify({
            prompt: 'Question for alpha',
            repo_scope: ['repo-1'],
          })
        );
        return delayedAsk.promise;
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Choose a repository scope and submit a question to the ask completions API.')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Question'), { target: { value: 'Question for alpha' } });
    fireEvent.click(screen.getByRole('button', { name: 'Ask' }));
    expect(screen.getByText('Loading answer…')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Repository scope'), { target: { value: 'repo-2' } });
    delayedAsk.resolve(
      jsonResponse({
        provider: 'test-provider',
        model: 'stale-model',
        answer: 'Stale alpha answer must be ignored.',
        citations: [],
        rendered_citations: [],
        thread_id: 'thread-1',
        session_id: 'session-1',
      })
    );

    await waitFor(() => {
      expect(window.location.hash).toBe('#/ask?repo_id=repo-2');
      expect(screen.queryByText('Stale alpha answer must be ignored.')).not.toBeInTheDocument();
      expect(screen.queryByText('Active thread: thread-1')).not.toBeInTheDocument();
    });
  });

  it('renders a dedicated chat route, loads thread summaries from the hash-selected thread, and stays on chat route', async () => {
    window.location.hash = '#/chat?thread_id=thread-2';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input);

      if (url === '/api/v1/repos') {
        return jsonResponse([
          {
            id: 'repo-1',
            name: 'alpha-repo',
            default_branch: 'main',
            sync_state: 'ready',
          },
          {
            id: 'repo-2',
            name: 'beta-repo',
            default_branch: 'main',
            sync_state: 'ready',
          },
        ]);
      }

      if (url === '/api/v1/ask/threads') {
        return jsonResponse([
          {
            id: 'thread-1',
            session_id: 'session-1',
            title: 'How does alpha work?',
            repo_scope: ['repo-1'],
            visibility: 'private',
            updated_at: '2026-04-21T20:00:00Z',
            message_count: 2,
          },
          {
            id: 'thread-2',
            session_id: 'session-2',
            title: 'Explain the routing flow',
            repo_scope: ['repo-2'],
            visibility: 'private',
            updated_at: '2026-04-21T21:00:00Z',
            message_count: 4,
          },
        ]);
      }

      if (url === '/api/v1/ask/threads/thread-2') {
        return jsonResponse({
          id: 'thread-2',
          session_id: 'session-2',
          user_id: 'user-1',
          title: 'Explain the routing flow',
          repo_scope: ['repo-2'],
          visibility: 'private',
          created_at: '2026-04-21T18:30:00Z',
          updated_at: '2026-04-21T21:00:00Z',
          messages: [
            {
              id: 'msg-1',
              role: 'user',
              content: 'How does the routing flow work?',
              citations: [],
              rendered_citations: [],
            },
            {
              id: 'msg-2',
              role: 'assistant',
              content: 'The chat route should restore the selected thread.',
              citations: [],
              rendered_citations: [],
            },
          ],
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(screen.getByRole('link', { name: 'Chat' })).toHaveAttribute('href', '#/chat');
    expect(screen.getByText('Browse and continue grounded ask threads from a dedicated chat route.')).toBeInTheDocument();
    expect(await screen.findByRole('button', { name: /Explain the routing flow/ })).toBeInTheDocument();
    expect(await screen.findByText('How does the routing flow work?')).toBeInTheDocument();
    expect(screen.getByText('Current scope: beta-repo')).toBeInTheDocument();
    expect(screen.queryByText('Ask grounded questions across visible repositories from a dedicated route.')).not.toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'Repositories' })).not.toBeInTheDocument();
    expect(window.location.hash).toBe('#/chat?repo_id=repo-2&thread_id=thread-2');
  });

  it('chat route updates the selected thread title and visibility through the PATCH lifecycle API', async () => {
    window.location.hash = '#/chat?thread_id=thread-2';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/repos') {
        return jsonResponse([
          {
            id: 'repo-2',
            name: 'beta-repo',
            default_branch: 'main',
            sync_state: 'ready',
          },
        ]);
      }

      if (url === '/api/v1/ask/threads') {
        return jsonResponse([
          {
            id: 'thread-2',
            session_id: 'session-2',
            title: 'Explain the routing flow',
            repo_scope: ['repo-2'],
            visibility: 'private',
            updated_at: '2026-04-21T21:00:00Z',
            message_count: 4,
          },
        ]);
      }

      if (url === '/api/v1/ask/threads/thread-2' && (!init || init.method === undefined)) {
        return jsonResponse({
          id: 'thread-2',
          session_id: 'session-2',
          user_id: 'user-1',
          title: 'Explain the routing flow',
          repo_scope: ['repo-2'],
          visibility: 'private',
          created_at: '2026-04-21T18:30:00Z',
          updated_at: '2026-04-21T21:00:00Z',
          messages: [
            {
              id: 'msg-1',
              role: 'user',
              content: 'How does the routing flow work?',
              citations: [],
              rendered_citations: [],
            },
          ],
        });
      }

      if (url === '/api/v1/ask/threads/thread-2' && init?.method === 'PATCH') {
        expect(init.body).toBe(
          JSON.stringify({
            title: 'Renamed routing thread',
            visibility: 'shared',
          })
        );

        return jsonResponse({
          id: 'thread-2',
          session_id: 'session-2',
          user_id: 'user-1',
          title: 'Renamed routing thread',
          repo_scope: ['repo-2'],
          visibility: 'shared',
          created_at: '2026-04-21T18:30:00Z',
          updated_at: '2026-04-21T22:00:00Z',
          messages: [
            {
              id: 'msg-1',
              role: 'user',
              content: 'How does the routing flow work?',
              citations: [],
              rendered_citations: [],
            },
          ],
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('How does the routing flow work?')).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText('Thread title'), { target: { value: 'Renamed routing thread' } });
    fireEvent.change(screen.getByLabelText('Thread visibility'), { target: { value: 'shared' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save thread metadata' }));

    expect(await screen.findByText('Thread metadata saved.')).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Renamed routing thread' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Renamed routing thread/ })).toBeInTheDocument();
    expect(screen.getByText('Visibility: shared')).toBeInTheDocument();
    expect(fetchMock.mock.calls.filter(([input, init]) => String(input) === '/api/v1/ask/threads/thread-2' && init?.method === 'PATCH')).toHaveLength(1);
    expect(window.location.hash).toBe('#/chat?repo_id=repo-2&thread_id=thread-2');
  });

  it('chat route deletes the selected thread, clears the hash thread id, and removes it from the list', async () => {
    window.location.hash = '#/chat?thread_id=thread-2';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/repos') {
        return jsonResponse([{ id: 'repo-2', name: 'beta-repo', default_branch: 'main', sync_state: 'ready' }]);
      }

      if (url === '/api/v1/ask/threads') {
        return jsonResponse([
          {
            id: 'thread-2',
            session_id: 'session-2',
            title: 'Thread to delete',
            repo_scope: ['repo-2'],
            visibility: 'private',
            updated_at: '2026-04-21T21:00:00Z',
            message_count: 1,
          },
        ]);
      }

      if (url === '/api/v1/ask/threads/thread-2' && init?.method === 'DELETE') {
        return new Response(null, { status: 204 });
      }

      if (url === '/api/v1/ask/threads/thread-2') {
        return jsonResponse({
          id: 'thread-2',
          session_id: 'session-2',
          user_id: 'user-1',
          title: 'Thread to delete',
          repo_scope: ['repo-2'],
          visibility: 'private',
          created_at: '2026-04-21T18:30:00Z',
          updated_at: '2026-04-21T21:00:00Z',
          messages: [{ id: 'msg-2', role: 'user', content: 'Delete me', citations: [], rendered_citations: [] }],
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Delete me')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Delete thread' }));

    await waitFor(() => expect(screen.queryByText('Delete me')).not.toBeInTheDocument());
    expect(screen.queryByRole('button', { name: /Thread to delete/ })).not.toBeInTheDocument();
    expect(screen.queryByText('Active thread: thread-2')).not.toBeInTheDocument();
    expect(screen.getByText('Thread deleted.')).toBeInTheDocument();
    expect(window.location.hash).toBe('#/chat?repo_id=repo-2');
    expect(fetchMock.mock.calls.filter(([input, init]) => String(input) === '/api/v1/ask/threads/thread-2' && init?.method === 'DELETE')).toHaveLength(1);
  });

  it('chat route ignores stale delete responses after switching to a different thread', async () => {
    window.location.hash = '#/chat?thread_id=thread-1';

    const delayedDelete = deferredResponse();

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/repos') {
        return jsonResponse([
          { id: 'repo-1', name: 'alpha-repo', default_branch: 'main', sync_state: 'ready' },
          { id: 'repo-2', name: 'beta-repo', default_branch: 'main', sync_state: 'ready' },
        ]);
      }

      if (url === '/api/v1/ask/threads') {
        return jsonResponse([
          { id: 'thread-1', session_id: 'session-1', title: 'Alpha thread', repo_scope: ['repo-1'], visibility: 'private', updated_at: '2026-04-21T20:00:00Z', message_count: 1 },
          { id: 'thread-2', session_id: 'session-2', title: 'Beta thread', repo_scope: ['repo-2'], visibility: 'private', updated_at: '2026-04-21T21:00:00Z', message_count: 1 },
        ]);
      }

      if (url === '/api/v1/ask/threads/thread-1' && init?.method === 'DELETE') {
        return delayedDelete.promise;
      }

      if (url === '/api/v1/ask/threads/thread-1') {
        return jsonResponse({
          id: 'thread-1',
          session_id: 'session-1',
          user_id: 'user-1',
          title: 'Alpha thread',
          repo_scope: ['repo-1'],
          visibility: 'private',
          created_at: '2026-04-21T18:00:00Z',
          updated_at: '2026-04-21T20:00:00Z',
          messages: [{ id: 'msg-1', role: 'user', content: 'Alpha question', citations: [], rendered_citations: [] }],
        });
      }

      if (url === '/api/v1/ask/threads/thread-2') {
        return jsonResponse({
          id: 'thread-2',
          session_id: 'session-2',
          user_id: 'user-1',
          title: 'Beta thread',
          repo_scope: ['repo-2'],
          visibility: 'private',
          created_at: '2026-04-21T18:30:00Z',
          updated_at: '2026-04-21T21:00:00Z',
          messages: [{ id: 'msg-2', role: 'user', content: 'Beta question', citations: [], rendered_citations: [] }],
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Alpha question')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Delete thread' }));
    expect(screen.getByRole('button', { name: 'Deleting…' })).toBeDisabled();
    fireEvent.click(screen.getByRole('button', { name: /Beta thread/ }));
    expect(await screen.findByText('Beta question')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Delete thread' })).not.toBeDisabled();

    delayedDelete.resolve(new Response(null, { status: 204 }));

    await waitFor(() => expect(screen.queryByText('Thread deleted.')).not.toBeInTheDocument());
    expect(screen.getByRole('heading', { name: 'Beta thread' })).toBeInTheDocument();
    expect(screen.getByText('Beta question')).toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'Alpha thread' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Delete thread' })).not.toBeDisabled();
    expect(window.location.hash).toBe('#/chat?repo_id=repo-2&thread_id=thread-2');
  });

  it('chat route ignores stale thread metadata responses after switching to a different thread', async () => {
    window.location.hash = '#/chat?thread_id=thread-1';

    const delayedMetadataUpdate = deferredResponse();

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/repos') {
        return jsonResponse([
          { id: 'repo-1', name: 'alpha-repo', default_branch: 'main', sync_state: 'ready' },
          { id: 'repo-2', name: 'beta-repo', default_branch: 'main', sync_state: 'ready' },
        ]);
      }

      if (url === '/api/v1/ask/threads') {
        return jsonResponse([
          { id: 'thread-1', session_id: 'session-1', title: 'Alpha thread', repo_scope: ['repo-1'], visibility: 'private', updated_at: '2026-04-21T20:00:00Z', message_count: 1 },
          { id: 'thread-2', session_id: 'session-2', title: 'Beta thread', repo_scope: ['repo-2'], visibility: 'private', updated_at: '2026-04-21T21:00:00Z', message_count: 1 },
        ]);
      }

      if (url === '/api/v1/ask/threads/thread-1' && init?.method === 'PATCH') {
        return delayedMetadataUpdate.promise;
      }

      if (url === '/api/v1/ask/threads/thread-1') {
        return jsonResponse({
          id: 'thread-1',
          session_id: 'session-1',
          user_id: 'user-1',
          title: 'Alpha thread',
          repo_scope: ['repo-1'],
          visibility: 'private',
          created_at: '2026-04-21T18:00:00Z',
          updated_at: '2026-04-21T20:00:00Z',
          messages: [{ id: 'msg-1', role: 'user', content: 'Alpha question', citations: [], rendered_citations: [] }],
        });
      }

      if (url === '/api/v1/ask/threads/thread-2') {
        return jsonResponse({
          id: 'thread-2',
          session_id: 'session-2',
          user_id: 'user-1',
          title: 'Beta thread',
          repo_scope: ['repo-2'],
          visibility: 'private',
          created_at: '2026-04-21T18:30:00Z',
          updated_at: '2026-04-21T21:00:00Z',
          messages: [{ id: 'msg-2', role: 'user', content: 'Beta question', citations: [], rendered_citations: [] }],
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Alpha question')).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText('Thread title'), { target: { value: 'Renamed stale alpha thread' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save thread metadata' }));
    fireEvent.click(screen.getByRole('button', { name: /Beta thread/ }));
    expect(await screen.findByText('Beta question')).toBeInTheDocument();

    delayedMetadataUpdate.resolve(
      jsonResponse({
        id: 'thread-1',
        session_id: 'session-1',
        user_id: 'user-1',
        title: 'Renamed stale alpha thread',
        repo_scope: ['repo-1'],
        visibility: 'shared',
        created_at: '2026-04-21T18:00:00Z',
        updated_at: '2026-04-21T22:00:00Z',
        messages: [{ id: 'msg-1', role: 'user', content: 'Alpha question', citations: [], rendered_citations: [] }],
      })
    );

    await waitFor(() => expect(screen.queryByText('Thread metadata saved.')).not.toBeInTheDocument());
    expect(screen.getByRole('heading', { name: 'Beta thread' })).toBeInTheDocument();
    expect(screen.getByText('Beta question')).toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'Renamed stale alpha thread' })).not.toBeInTheDocument();
    expect(window.location.hash).toBe('#/chat?repo_id=repo-2&thread_id=thread-2');
  });

  it('submitting from the chat route continues the selected thread, appends the new answer, and keeps the hash pinned', async () => {
    window.location.hash = '#/chat?thread_id=thread-2';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/repos') {
        return jsonResponse([
          {
            id: 'repo-1',
            name: 'alpha-repo',
            default_branch: 'main',
            sync_state: 'ready',
          },
          {
            id: 'repo-2',
            name: 'beta-repo',
            default_branch: 'main',
            sync_state: 'ready',
          },
        ]);
      }

      if (url === '/api/v1/ask/threads') {
        return jsonResponse([
          {
            id: 'thread-2',
            session_id: 'session-2',
            title: 'Explain the routing flow',
            repo_scope: ['repo-2'],
            visibility: 'private',
            updated_at: '2026-04-21T21:00:00Z',
            message_count: 2,
          },
        ]);
      }

      if (url === '/api/v1/ask/threads/thread-2') {
        return jsonResponse({
          id: 'thread-2',
          session_id: 'session-2',
          user_id: 'user-1',
          title: 'Explain the routing flow',
          repo_scope: ['repo-2'],
          visibility: 'private',
          created_at: '2026-04-21T18:30:00Z',
          updated_at: '2026-04-21T21:00:00Z',
          messages: [
            {
              id: 'msg-1',
              role: 'user',
              content: 'What keeps the route pinned?',
              citations: [],
              rendered_citations: [],
            },
          ],
        });
      }

      if (url === '/api/v1/ask/completions') {
        expect(init?.body).toBe(
          JSON.stringify({
            prompt: 'Continue the chat thread',
            repo_scope: ['repo-2'],
            thread_id: 'thread-2',
          })
        );

        return jsonResponse({
          provider: 'test-provider',
          model: 'chat-model',
          answer: 'The selected thread id remains in the chat hash.',
          citations: [],
          rendered_citations: [],
          thread_id: 'thread-2',
          session_id: 'session-2',
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('What keeps the route pinned?')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Question'), { target: { value: 'Continue the chat thread' } });
    fireEvent.click(screen.getByRole('button', { name: 'Ask' }));

    expect(await screen.findByText('The selected thread id remains in the chat hash.')).toBeInTheDocument();
    expect(window.location.hash).toBe('#/chat?repo_id=repo-2&thread_id=thread-2');
    expect(fetchMock.mock.calls.filter(([input]) => String(input) === '/api/v1/ask/completions')).toHaveLength(1);
  });

  it('chat route clears a stale restored thread after a 404 follow-up and resets the hash to a fresh chat baseline', async () => {
    window.location.hash = '#/chat?thread_id=thread-stale';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/repos') {
        return jsonResponse([
          {
            id: 'repo-1',
            name: 'alpha-repo',
            default_branch: 'main',
            sync_state: 'ready',
          },
        ]);
      }

      if (url === '/api/v1/ask/threads') {
        return jsonResponse([
          {
            id: 'thread-stale',
            session_id: 'session-stale',
            title: 'Stale thread',
            repo_scope: ['repo-1'],
            visibility: 'private',
            updated_at: '2026-04-21T21:00:00Z',
            message_count: 2,
          },
        ]);
      }

      if (url === '/api/v1/ask/threads/thread-stale') {
        return jsonResponse({
          id: 'thread-stale',
          session_id: 'session-stale',
          user_id: 'user-1',
          title: 'Stale thread',
          repo_scope: ['repo-1'],
          visibility: 'private',
          created_at: '2026-04-21T18:30:00Z',
          updated_at: '2026-04-21T21:00:00Z',
          messages: [
            {
              id: 'msg-1',
              role: 'user',
              content: 'Stale question',
              citations: [],
              rendered_citations: [],
            },
          ],
        });
      }

      if (url === '/api/v1/ask/completions') {
        expect(init?.body).toBe(
          JSON.stringify({
            prompt: 'Resume the stale chat thread',
            repo_scope: ['repo-1'],
            thread_id: 'thread-stale',
          })
        );
        return jsonResponse({ error: 'missing thread' }, false, 404);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Active thread: thread-stale')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Question'), { target: { value: 'Resume the stale chat thread' } });
    fireEvent.click(screen.getByRole('button', { name: 'Ask' }));

    expect(await screen.findByText(/The restored chat thread is no longer available for this repository scope\. Start a fresh chat\./)).toBeInTheDocument();
    expect(screen.queryByText('Active thread: thread-stale')).not.toBeInTheDocument();
    expect(window.location.hash).toBe('#/chat?repo_id=repo-1');
  });

  it('chat route invalidates an in-flight ask when switching to a different thread', async () => {
    window.location.hash = '#/chat?thread_id=thread-1';

    const delayedFirstThreadAsk = deferredResponse();

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/repos') {
        return jsonResponse([
          {
            id: 'repo-1',
            name: 'alpha-repo',
            default_branch: 'main',
            sync_state: 'ready',
          },
          {
            id: 'repo-2',
            name: 'beta-repo',
            default_branch: 'main',
            sync_state: 'ready',
          },
        ]);
      }

      if (url === '/api/v1/ask/threads') {
        return jsonResponse([
          {
            id: 'thread-1',
            session_id: 'session-1',
            title: 'Alpha thread',
            repo_scope: ['repo-1'],
            visibility: 'private',
            updated_at: '2026-04-21T20:00:00Z',
            message_count: 2,
          },
          {
            id: 'thread-2',
            session_id: 'session-2',
            title: 'Beta thread',
            repo_scope: ['repo-2'],
            visibility: 'private',
            updated_at: '2026-04-21T21:00:00Z',
            message_count: 2,
          },
        ]);
      }

      if (url === '/api/v1/ask/threads/thread-1') {
        return jsonResponse({
          id: 'thread-1',
          session_id: 'session-1',
          user_id: 'user-1',
          title: 'Alpha thread',
          repo_scope: ['repo-1'],
          visibility: 'private',
          created_at: '2026-04-21T18:00:00Z',
          updated_at: '2026-04-21T20:00:00Z',
          messages: [
            {
              id: 'msg-1',
              role: 'user',
              content: 'How does alpha work?',
              citations: [],
              rendered_citations: [],
            },
          ],
        });
      }

      if (url === '/api/v1/ask/threads/thread-2') {
        return jsonResponse({
          id: 'thread-2',
          session_id: 'session-2',
          user_id: 'user-1',
          title: 'Beta thread',
          repo_scope: ['repo-2'],
          visibility: 'private',
          created_at: '2026-04-21T18:30:00Z',
          updated_at: '2026-04-21T21:00:00Z',
          messages: [
            {
              id: 'msg-2',
              role: 'user',
              content: 'How does beta work?',
              citations: [],
              rendered_citations: [],
            },
          ],
        });
      }

      if (url === '/api/v1/ask/completions') {
        if (String(init?.body).includes('thread-1')) {
          return delayedFirstThreadAsk.promise;
        }

        expect(init?.body).toBe(
          JSON.stringify({
            prompt: 'Continue the beta thread',
            repo_scope: ['repo-2'],
            thread_id: 'thread-2',
          })
        );
        return jsonResponse({
          provider: 'test-provider',
          model: 'chat-model',
          answer: 'The second thread keeps its own repo scope.',
          citations: [],
          rendered_citations: [],
          thread_id: 'thread-2',
          session_id: 'session-2',
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Active thread: thread-1')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Question'), { target: { value: 'Continue alpha' } });
    fireEvent.click(screen.getByRole('button', { name: 'Ask' }));

    fireEvent.click(screen.getByRole('button', { name: /Beta thread/ }));
    expect(await screen.findByText('Current scope: beta-repo')).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Ask' })).toBeEnabled();
    });

    fireEvent.change(screen.getByLabelText('Question'), { target: { value: 'Continue the beta thread' } });
    fireEvent.click(screen.getByRole('button', { name: 'Ask' }));

    expect(await screen.findByText('The second thread keeps its own repo scope.')).toBeInTheDocument();
    expect(window.location.hash).toBe('#/chat?repo_id=repo-2&thread_id=thread-2');

    delayedFirstThreadAsk.resolve(
      jsonResponse({
        provider: 'test-provider',
        model: 'chat-model',
        answer: 'This stale answer should not come back.',
        citations: [],
        rendered_citations: [],
        thread_id: 'thread-1',
        session_id: 'session-1',
      })
    );

    await waitFor(() => {
      expect(screen.queryByText('This stale answer should not come back.')).not.toBeInTheDocument();
    });
    expect(screen.getByText('Current scope: beta-repo')).toBeInTheDocument();
  });

  it('renders an auth route that supports first-run onboarding and then local login', async () => {
    window.location.hash = '#/auth';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/bootstrap' && !init?.method) {
        return jsonResponse({ bootstrap_required: true });
      }

      if (url === '/api/v1/auth/bootstrap' && init?.method === 'POST') {
        expect(init.body).toBe(JSON.stringify({
          name: 'First Admin',
          email: 'admin@example.com',
          password: 'super-secret',
        }));

        return jsonResponse({ bootstrap_required: false });
      }

      if (url === '/api/v1/auth/login') {
        expect(init?.method).toBe('POST');
        expect(init?.body).toBe(JSON.stringify({
          email: 'admin@example.com',
          password: 'super-secret',
        }));

        return jsonResponse({
          session_id: 'session-1',
          session_secret: 'secret-1',
          user_id: 'user-1',
          created_at: '2026-04-18T12:00:00Z',
        });
      }

      if (url === '/api/v1/auth/me') {
        expect(init?.headers).toMatchObject({
          Authorization: 'Bearer session-1:secret-1',
        });

        return jsonResponse({
          user_id: 'user-1',
          email: 'admin@example.com',
          name: 'First Admin',
          session_id: 'session-1',
          created_at: '2026-04-18T12:00:00Z',
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByRole('heading', { name: 'First-run onboarding' })).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'First Admin' } });
    fireEvent.change(screen.getByLabelText('Email address'), { target: { value: 'admin@example.com' } });
    fireEvent.change(screen.getByLabelText('Password'), { target: { value: 'super-secret' } });
    fireEvent.click(screen.getByRole('button', { name: 'Create first admin account' }));

    expect(await screen.findByRole('heading', { name: 'Local login' })).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Email address'), { target: { value: 'admin@example.com' } });
    fireEvent.change(screen.getByLabelText('Password'), { target: { value: 'super-secret' } });
    fireEvent.click(screen.getByRole('button', { name: 'Sign in locally' }));

    expect(await screen.findByText('Signed in as First Admin')).toBeInTheDocument();
    expect(screen.getByText('admin@example.com')).toBeInTheDocument();
    expect(window.localStorage.getItem('sourcebot-local-session')).toBe(
      JSON.stringify({ sessionId: 'session-1', sessionSecret: 'secret-1' })
    );
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/auth/bootstrap');
  });

  it('restores an existing local session on the auth route and sends it to protected auth endpoints', async () => {
    window.location.hash = '#/auth';
    window.localStorage.setItem(
      'sourcebot-local-session',
      JSON.stringify({ sessionId: 'session-9', sessionSecret: 'secret-9' })
    );

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/me') {
        expect(init?.headers).toMatchObject({
          Authorization: 'Bearer session-9:secret-9',
        });

        return jsonResponse({
          user_id: 'user-9',
          email: 'admin@example.com',
          name: 'Restored Admin',
          session_id: 'session-9',
          created_at: '2026-04-18T12:00:00Z',
        });
      }

      if (url === '/api/v1/auth/api-keys') {
        expect(init?.headers).toMatchObject({
          Authorization: 'Bearer session-9:secret-9',
        });

        return jsonResponse([]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Signed in as Restored Admin')).toBeInTheDocument();

    window.location.hash = '#/settings/api-keys';

    expect(await screen.findByText('No API keys found')).toBeInTheDocument();
    expect(screen.getByText('Use the create form above for the current minimal flow;', { exact: false })).toBeInTheDocument();
    expect(fetchMock).toHaveBeenCalledWith(
      '/api/v1/auth/api-keys',
      expect.objectContaining({
        headers: expect.objectContaining({
          Authorization: 'Bearer session-9:secret-9',
        }),
      })
    );
  });

  it('keeps oauth callback status visible when an existing local session is restored on the auth route', async () => {
    window.location.hash = '#/auth?provider=github&error=access_denied';
    window.localStorage.setItem(
      'sourcebot-local-session',
      JSON.stringify({ sessionId: 'session-9', sessionSecret: 'secret-9' })
    );

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/me') {
        expect(init?.headers).toMatchObject({
          Authorization: 'Bearer session-9:secret-9',
        });

        return jsonResponse({
          user_id: 'user-9',
          email: 'admin@example.com',
          name: 'Restored Admin',
          session_id: 'session-9',
          created_at: '2026-04-18T12:00:00Z',
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Signed in as Restored Admin')).toBeInTheDocument();
    expect(screen.getByText('OAuth callback parameters for GitHub indicate sign-in did not complete.')).toBeInTheDocument();
    expect(screen.getByText('OAuth error: access_denied')).toBeInTheDocument();
  });

  it('logs out from the auth route, clears the stored session, and returns to local login', async () => {
    window.location.hash = '#/auth';
    window.localStorage.setItem(
      'sourcebot-local-session',
      JSON.stringify({ sessionId: 'session-3', sessionSecret: 'secret-3' })
    );

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/me') {
        return jsonResponse({
          user_id: 'user-3',
          email: 'admin@example.com',
          name: 'Logged In Admin',
          session_id: 'session-3',
          created_at: '2026-04-18T12:00:00Z',
        });
      }

      if (url === '/api/v1/auth/logout') {
        expect(init?.method).toBe('POST');
        expect(init?.headers).toMatchObject({
          Authorization: 'Bearer session-3:secret-3',
        });
        return jsonResponse({ success: true });
      }

      if (url === '/api/v1/auth/bootstrap') {
        return jsonResponse({ bootstrap_required: false });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Signed in as Logged In Admin')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Log out' }));

    expect(await screen.findByRole('heading', { name: 'Local login' })).toBeInTheDocument();
    expect(window.localStorage.getItem('sourcebot-local-session')).toBeNull();
  });

  it('renders invite redemption on the auth route, stores the returned session, and restores the invited identity', async () => {
    window.location.hash = '#/auth?invite=invite-7&email=invitee%40example.com';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/bootstrap') {
        return jsonResponse({ bootstrap_required: false });
      }

      if (url === '/api/v1/auth/invite-redeem') {
        expect(init?.method).toBe('POST');
        expect(init?.body).toBe(JSON.stringify({
          invite_id: 'invite-7',
          email: 'invitee@example.com',
          name: 'Invitee Person',
          password: 'invite-password',
        }));

        return jsonResponse({
          session_id: 'invite-session-1',
          session_secret: 'invite-secret-1',
          user_id: 'user-invitee',
          created_at: '2026-04-21T12:00:00Z',
        }, true, 201);
      }

      if (url === '/api/v1/auth/me') {
        expect(init?.headers).toMatchObject({
          Authorization: 'Bearer invite-session-1:invite-secret-1',
        });

        return jsonResponse({
          user_id: 'user-invitee',
          email: 'invitee@example.com',
          name: 'Invitee Person',
          session_id: 'invite-session-1',
          created_at: '2026-04-21T12:00:00Z',
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByRole('heading', { name: 'Invite redemption' })).toBeInTheDocument();
    expect(screen.getByDisplayValue('invitee@example.com')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'Invitee Person' } });
    fireEvent.change(screen.getByLabelText('Password'), { target: { value: 'invite-password' } });
    fireEvent.click(screen.getByRole('button', { name: 'Accept invite and sign in' }));

    expect(await screen.findByText('Signed in as Invitee Person')).toBeInTheDocument();
    expect(window.localStorage.getItem('sourcebot-local-session')).toBe(
      JSON.stringify({ sessionId: 'invite-session-1', sessionSecret: 'invite-secret-1' })
    );
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/auth/bootstrap');
  });

  it('keeps oauth callback status visible alongside invite redemption on the auth route', async () => {
    window.location.hash = '#/auth?invite=invite-7&email=invitee%40example.com&provider=github&error=access_denied';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input);

      if (url === '/api/v1/auth/bootstrap') {
        return jsonResponse({ bootstrap_required: false });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByRole('heading', { name: 'Invite redemption' })).toBeInTheDocument();
    expect(screen.getByText('OAuth callback parameters for GitHub indicate sign-in did not complete.')).toBeInTheDocument();
    expect(screen.getByText('OAuth error: access_denied')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Accept invite and sign in' })).toBeInTheDocument();
  });

  it('shows a truthful auth error when invite redemption fails and keeps the session cleared', async () => {
    window.location.hash = '#/auth?invite=invite-7&email=invitee%40example.com';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/bootstrap') {
        return jsonResponse({ bootstrap_required: false });
      }

      if (url === '/api/v1/auth/invite-redeem') {
        expect(init?.method).toBe('POST');
        return jsonResponse({ error: 'unauthorized' }, false, 401);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByRole('heading', { name: 'Invite redemption' })).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'Invitee Person' } });
    fireEvent.change(screen.getByLabelText('Password'), { target: { value: 'invite-password' } });
    fireEvent.click(screen.getByRole('button', { name: 'Accept invite and sign in' }));

    expect(await screen.findByText('Authentication error: Request failed: 401')).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Invite redemption' })).toBeInTheDocument();
    expect(window.localStorage.getItem('sourcebot-local-session')).toBeNull();
  });

  it('lets an invited local account return through the standard auth login form after invite redemption', async () => {
    window.location.hash = '#/auth';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/bootstrap') {
        return jsonResponse({ bootstrap_required: false });
      }

      if (url === '/api/v1/auth/login') {
        expect(init?.method).toBe('POST');
        expect(init?.body).toBe(JSON.stringify({
          email: 'invitee@example.com',
          password: 'invite-password',
        }));

        return jsonResponse({
          session_id: 'invite-login-session',
          session_secret: 'invite-login-secret',
          user_id: 'user-invitee',
          created_at: '2026-04-21T12:05:00Z',
        }, true, 201);
      }

      if (url === '/api/v1/auth/me') {
        expect(init?.headers).toMatchObject({
          Authorization: 'Bearer invite-login-session:invite-login-secret',
        });

        return jsonResponse({
          user_id: 'user-invitee',
          email: 'invitee@example.com',
          name: 'Invitee Person',
          session_id: 'invite-login-session',
          created_at: '2026-04-21T12:05:00Z',
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByRole('heading', { name: 'Local login' })).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText('Email address'), { target: { value: 'invitee@example.com' } });
    fireEvent.change(screen.getByLabelText('Password'), { target: { value: 'invite-password' } });
    fireEvent.click(screen.getByRole('button', { name: 'Sign in locally' }));

    expect(await screen.findByText('Signed in as Invitee Person')).toBeInTheDocument();
    expect(screen.getByText('invitee@example.com')).toBeInTheDocument();
    expect(window.localStorage.getItem('sourcebot-local-session')).toBe(
      JSON.stringify({ sessionId: 'invite-login-session', sessionSecret: 'invite-login-secret' })
    );
  });

  it('renders an oauth callback error status on the auth route while keeping local login available', async () => {
    window.location.hash = '#/auth?provider=github&error=access_denied&error_description=Org%20SSO%20required';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input);

      if (url === '/api/v1/auth/bootstrap') {
        return jsonResponse({ bootstrap_required: false });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByRole('heading', { name: 'Local login' })).toBeInTheDocument();
    expect(screen.getByText('OAuth callback parameters for GitHub indicate sign-in did not complete.')).toBeInTheDocument();
    expect(screen.getByText('OAuth error: access_denied')).toBeInTheDocument();
    expect(screen.getByText('Org SSO required')).toBeInTheDocument();
    expect(screen.getByText('This rewrite does not finish external-provider sign-in on this screen yet.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Sign in locally' })).toBeInTheDocument();
  });

  it('renders an oauth callback provider status when only the provider param is present', async () => {
    window.location.hash = '#/auth?provider=github';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input);

      if (url === '/api/v1/auth/bootstrap') {
        return jsonResponse({ bootstrap_required: false });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByRole('heading', { name: 'Local login' })).toBeInTheDocument();
    expect(screen.getByText('This auth route received OAuth callback parameters for GitHub.')).toBeInTheDocument();
    expect(screen.getByText('Provider: GitHub')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Sign in locally' })).toBeInTheDocument();
  });

  it('renders an oauth callback code status on the auth route while keeping local login available without echoing secret callback values', async () => {
    window.location.hash = '#/auth?provider=github&code=callback-code&state=opaque-state';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input);

      if (url === '/api/v1/auth/bootstrap') {
        return jsonResponse({ bootstrap_required: false });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByRole('heading', { name: 'Local login' })).toBeInTheDocument();
    expect(screen.getByText('This auth route received OAuth callback parameters for GitHub.')).toBeInTheDocument();
    expect(screen.getByText('This rewrite does not finish external-provider sign-in here yet, so use local login below if you need access right now.')).toBeInTheDocument();
    expect(screen.getByText('An authorization code was received, but it is not exchanged on this screen yet.')).toBeInTheDocument();
    expect(screen.getByText('An OAuth state parameter was also present.')).toBeInTheDocument();
    expect(screen.queryByText('Authorization code received: callback-code')).not.toBeInTheDocument();
    expect(screen.queryByText('State received: opaque-state')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Sign in locally' })).toBeInTheDocument();
  });

  it('keeps first-run onboarding available when oauth callback params land on an unbootstrapped auth route', async () => {
    window.location.hash = '#/auth?provider=github&error=access_denied';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input);

      if (url === '/api/v1/auth/bootstrap') {
        return jsonResponse({ bootstrap_required: true });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByRole('heading', { name: 'First-run onboarding' })).toBeInTheDocument();
    expect(screen.getByText('OAuth callback parameters for GitHub indicate sign-in did not complete.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Create first admin account' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Sign in locally' })).not.toBeInTheDocument();
  });

  it('keeps the repository home focused on repository inventory while linking to the dedicated search route', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
      jsonResponse([
        {
          id: 'repo-1',
          name: 'alpha-repo',
          default_branch: 'main',
          sync_state: 'ready',
        },
      ])
    );

    render(<App />);

    expect(await screen.findByText('Default branch: main')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Search' })).toHaveAttribute('href', '#/search');
    expect(screen.getByRole('link', { name: 'Open dedicated search page' })).toHaveAttribute('href', '#/search');
    expect(screen.queryByLabelText('Search query')).not.toBeInTheDocument();
  });

  it('loads repository detail and browses directories and files from the browse api', async () => {
    window.location.hash = '#/repos/repo-42';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input);

      if (url === '/api/v1/repos/repo-42') {
        return jsonResponse({
          repository: {
            id: 'repo-42',
            name: 'beta-repo',
            default_branch: 'develop',
            connection_id: 'conn-7',
            sync_state: 'pending',
          },
          connection: {
            id: 'conn-7',
            name: 'GitHub App',
            kind: 'github',
          },
        });
      }

      if (url === '/api/v1/repos/repo-42/tree?path=') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: '',
          entries: [
            { name: 'src', path: 'src', kind: 'dir' },
            { name: 'README.md', path: 'README.md', kind: 'file' },
          ],
        });
      }

      if (url === '/api/v1/repos/repo-42/tree?path=src') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'src',
          entries: [{ name: 'App.tsx', path: 'src/App.tsx', kind: 'file' }],
        });
      }

      if (url === '/api/v1/repos/repo-42/blob?path=src%2FApp.tsx') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'src/App.tsx',
          size_bytes: 42,
          content: 'export function App() { return null; }',
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('beta-repo')).toBeInTheDocument();
    expect(screen.getByText('Repository id: repo-42')).toBeInTheDocument();
    expect(screen.getByText('GitHub App')).toBeInTheDocument();
    const srcButton = (await screen.findByText('src/')).closest('button');
    const readmeButton = screen.getByText('README.md').closest('button');

    expect(srcButton).toBeInTheDocument();
    expect(readmeButton).toBeInTheDocument();

    fireEvent.click(srcButton!);

    expect(await screen.findByText('Current path: src')).toBeInTheDocument();
    const appFileButton = screen.getByText('App.tsx').closest('button');
    expect(appFileButton).toBeInTheDocument();

    fireEvent.click(appFileButton!);

    expect(await screen.findByText('src/App.tsx')).toBeInTheDocument();
    expect(screen.getByText('42 bytes')).toBeInTheDocument();
    expect(screen.getByText('export function App() { return null; }')).toBeInTheDocument();

    expect(fetchMock).toHaveBeenCalledWith('/api/v1/repos/repo-42/tree?path=');
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/repos/repo-42/tree?path=src');
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/repos/repo-42/blob?path=src%2FApp.tsx');
  });

  it('keeps branch or revision controls in the repo route and reloads browse plus commits for the selected revision', async () => {
    window.location.hash = '#/repos/repo-42?path=src%2Ffeature.ts&from=search&q=router&repo_id=repo-42&revision=feature%2Fdemo';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input);

      if (url === '/api/v1/repos/repo-42') {
        return jsonResponse({
          repository: {
            id: 'repo-42',
            name: 'beta-repo',
            default_branch: 'develop',
            connection_id: 'conn-7',
            sync_state: 'ready',
          },
          connection: {
            id: 'conn-7',
            name: 'GitHub App',
            kind: 'github',
          },
        });
      }

      if (url === '/api/v1/repos/repo-42/tree?path=src&revision=feature%2Fdemo') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'src',
          entries: [{ name: 'feature.ts', path: 'src/feature.ts', kind: 'file' }],
        });
      }

      if (url === '/api/v1/repos/repo-42/blob?path=src%2Ffeature.ts&revision=feature%2Fdemo') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'src/feature.ts',
          size_bytes: 18,
          content: 'feature revision\n',
        });
      }

      if (url === '/api/v1/repos/repo-42/commits?limit=20&revision=feature%2Fdemo') {
        return jsonResponse({
          repo_id: 'repo-42',
          commits: [
            {
              id: 'feature123',
              short_id: 'feature1',
              summary: 'Feature branch commit',
              author_name: 'Hermes Agent',
              authored_at: '2026-04-18T12:00:00Z',
            },
          ],
        });
      }

      if (url === '/api/v1/repos/repo-42/tree?path=src&revision=release%2F1.2') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'src',
          entries: [{ name: 'release.ts', path: 'src/release.ts', kind: 'file' }],
        });
      }

      if (url === '/api/v1/repos/repo-42/blob?path=src%2Frelease.ts&revision=release%2F1.2') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'src/release.ts',
          size_bytes: 18,
          content: 'release revision\n',
        });
      }

      if (url === '/api/v1/repos/repo-42/commits?limit=20&revision=release%2F1.2') {
        return jsonResponse({
          repo_id: 'repo-42',
          commits: [
            {
              id: 'release123',
              short_id: 'release',
              summary: 'Release branch commit',
              author_name: 'Hermes Agent',
              authored_at: '2026-04-18T13:00:00Z',
            },
          ],
        });
      }

      if (url === '/api/v1/repos/repo-42/tree?path=src&revision=develop') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'src',
          entries: [{ name: 'default.ts', path: 'src/default.ts', kind: 'file' }],
        });
      }

      if (url === '/api/v1/repos/repo-42/blob?path=src%2Fdefault.ts&revision=develop') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'src/default.ts',
          size_bytes: 17,
          content: 'default branch\n',
        });
      }

      if (url === '/api/v1/repos/repo-42/commits?limit=20&revision=develop') {
        return jsonResponse({
          repo_id: 'repo-42',
          commits: [],
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByDisplayValue('feature/demo')).toBeInTheDocument();
    expect(screen.getByText('Current path: src')).toBeInTheDocument();
    expect(screen.getByText('Viewing revision: feature/demo')).toBeInTheDocument();
    expect(await screen.findByText('Feature branch commit')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: '← Back to search results' })).toHaveAttribute('href', '#/search?q=router&repo_id=repo-42');

    fireEvent.change(screen.getByLabelText('Branch, tag, or revision'), { target: { value: 'release/1.2' } });
    fireEvent.click(screen.getByRole('button', { name: 'Apply revision' }));

    await screen.findByDisplayValue('release/1.2');
    expect(screen.getByDisplayValue('release/1.2')).toBeInTheDocument();
    await screen.findByText('Release branch commit');
    expect(screen.getByText('Release branch commit')).toBeInTheDocument();
    await waitFor(() => {
      expect(window.location.hash).toBe('#/repos/repo-42?path=src%2Frelease.ts&from=search&q=router&repo_id=repo-42&revision=release%2F1.2');
    }, { timeout: 4000 });
    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith('/api/v1/repos/repo-42/blob?path=src%2Frelease.ts&revision=release%2F1.2');
    }, { timeout: 4000 });
    expect(await screen.findByText('release revision', undefined, { timeout: 4000 })).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Reset to default branch' }));

    await waitFor(() => {
      expect(window.location.hash).toBe('#/repos/repo-42?path=src%2Fdefault.ts&from=search&q=router&repo_id=repo-42&revision=develop');
    }, { timeout: 4000 });

    expect(await screen.findByText('src/default.ts', undefined, { timeout: 4000 })).toBeInTheDocument();
    expect(await screen.findByText('default branch', undefined, { timeout: 4000 })).toBeInTheDocument();
    await waitFor(() => {
      expect(window.location.hash).toBe('#/repos/repo-42?path=src%2Fdefault.ts&from=search&q=router&repo_id=repo-42&revision=develop');
    }, { timeout: 4000 });
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/repos/repo-42/tree?path=src&revision=feature%2Fdemo');
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/repos/repo-42/blob?path=src%2Ffeature.ts&revision=feature%2Fdemo');
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/repos/repo-42/commits?limit=20&revision=feature%2Fdemo');
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/repos/repo-42/tree?path=src&revision=release%2F1.2');
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/repos/repo-42/blob?path=src%2Frelease.ts&revision=release%2F1.2');
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/repos/repo-42/commits?limit=20&revision=release%2F1.2');
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/repos/repo-42/tree?path=src&revision=develop');
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/repos/repo-42/blob?path=src%2Fdefault.ts&revision=develop');
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/repos/repo-42/commits?limit=20&revision=develop');
  });

  it('clears repo-scoped browse state when navigating to a different repository detail route', async () => {
    window.location.hash = '#/repos/repo-a?path=src%2Falpha.ts';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input);

      if (url === '/api/v1/repos/repo-a') {
        return jsonResponse({
          repository: {
            id: 'repo-a',
            name: 'repo-a',
            default_branch: 'main',
            connection_id: 'conn-a',
            sync_state: 'ready',
          },
          connection: {
            id: 'conn-a',
            name: 'Connection A',
            kind: 'github',
          },
        });
      }

      if (url === '/api/v1/repos/repo-a/tree?path=src') {
        return jsonResponse({
          repo_id: 'repo-a',
          path: 'src',
          entries: [{ name: 'alpha.ts', path: 'src/alpha.ts', kind: 'file' }],
        });
      }

      if (url === '/api/v1/repos/repo-a/blob?path=src%2Falpha.ts') {
        return jsonResponse({
          repo_id: 'repo-a',
          path: 'src/alpha.ts',
          size_bytes: 8,
          content: 'alpha A\n',
        });
      }

      if (url === '/api/v1/repos/repo-a/commits?limit=20') {
        return jsonResponse({ repo_id: 'repo-a', commits: [] });
      }

      if (url === '/api/v1/repos/repo-b') {
        return jsonResponse({
          repository: {
            id: 'repo-b',
            name: 'repo-b',
            default_branch: 'main',
            connection_id: 'conn-b',
            sync_state: 'ready',
          },
          connection: {
            id: 'conn-b',
            name: 'Connection B',
            kind: 'gitlab',
          },
        });
      }

      if (url === '/api/v1/repos/repo-b/tree?path=') {
        return jsonResponse({
          repo_id: 'repo-b',
          path: '',
          entries: [{ name: 'README.md', path: 'README.md', kind: 'file' }],
        });
      }

      if (url === '/api/v1/repos/repo-b/blob?path=README.md') {
        return jsonResponse({
          repo_id: 'repo-b',
          path: 'README.md',
          size_bytes: 7,
          content: 'repo B\n',
        });
      }

      if (url === '/api/v1/repos/repo-b/commits?limit=20') {
        return jsonResponse({ repo_id: 'repo-b', commits: [] });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('alpha A')).toBeInTheDocument();

    window.location.hash = '#/repos/repo-b';
    window.dispatchEvent(new HashChangeEvent('hashchange'));

    expect(await screen.findByText('repo-b')).toBeInTheDocument();
    expect(await screen.findByText('README.md')).toBeInTheDocument();
    expect(screen.getAllByText('Select a file to inspect its contents.').length).toBeGreaterThan(0);
    expect(screen.queryByText('alpha A')).not.toBeInTheDocument();
    expect(fetchMock).not.toHaveBeenCalledWith('/api/v1/repos/repo-b/tree?path=src');
    expect(fetchMock).not.toHaveBeenCalledWith('/api/v1/repos/repo-b/blob?path=src%2Falpha.ts');
  });

  it('preserves the search back link and can retry repository detail loading after an error', async () => {
    window.location.hash = '#/repos/repo-42?from=search&q=router&repo_id=repo-42';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input);

      if (url === '/api/v1/repos/repo-42') {
        fetchMock.mock.calls.filter(([calledUrl]) => calledUrl === '/api/v1/repos/repo-42');
        if (fetchMock.mock.calls.filter(([calledUrl]) => calledUrl === '/api/v1/repos/repo-42').length === 1) {
          return jsonResponse({}, false, 503);
        }

        return jsonResponse({
          repository: {
            id: 'repo-42',
            name: 'beta-repo',
            default_branch: 'develop',
            connection_id: 'conn-7',
            sync_state: 'pending',
          },
          connection: {
            id: 'conn-7',
            name: 'GitHub App',
            kind: 'github',
          },
        });
      }

      if (url === '/api/v1/repos/repo-42/tree?path=') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: '',
          entries: [],
        });
      }

      if (url === '/api/v1/repos/repo-42/commits?limit=20') {
        return jsonResponse({
          repo_id: 'repo-42',
          commits: [],
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Failed to load repository: Request failed: 503')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: '← Back to search results' })).toHaveAttribute('href', '#/search?q=router&repo_id=repo-42');
    fireEvent.click(screen.getByRole('button', { name: 'Retry loading repository' }));

    expect(await screen.findByText('beta-repo')).toBeInTheDocument();
    expect(screen.getByText('No commits found.')).toBeInTheDocument();
    expect(screen.getByText('This directory is empty.')).toBeInTheDocument();
    expect(fetchMock.mock.calls.filter(([calledUrl]) => calledUrl === '/api/v1/repos/repo-42')).toHaveLength(2);
  });

  it('shows browse errors without breaking repository detail rendering and can retry the tree request', async () => {
    window.location.hash = '#/repos/repo-42';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input);

      if (url === '/api/v1/repos/repo-42') {
        return jsonResponse({
          repository: {
            id: 'repo-42',
            name: 'beta-repo',
            default_branch: 'develop',
            connection_id: 'conn-7',
            sync_state: 'pending',
          },
          connection: {
            id: 'conn-7',
            name: 'GitHub App',
            kind: 'github',
          },
        });
      }

      if (url === '/api/v1/repos/repo-42/tree?path=') {
        if (fetchMock.mock.calls.filter(([calledUrl]) => calledUrl === '/api/v1/repos/repo-42/tree?path=').length === 1) {
          return jsonResponse({}, false, 503);
        }

        return jsonResponse({
          repo_id: 'repo-42',
          path: '',
          entries: [{ name: 'README.md', path: 'README.md', kind: 'file' }],
        });
      }

      if (url === '/api/v1/repos/repo-42/commits?limit=20') {
        return jsonResponse({
          repo_id: 'repo-42',
          commits: [],
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('beta-repo')).toBeInTheDocument();
    expect(await screen.findByText('Unable to load files: Request failed: 503')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Retry loading files' }));

    expect(await screen.findByText('README.md')).toBeInTheDocument();
    expect(fetchMock.mock.calls.filter(([calledUrl]) => calledUrl === '/api/v1/repos/repo-42/tree?path=')).toHaveLength(2);
  });

  it('loads recent commits and shows commit details when a commit is selected', async () => {
    window.location.hash = '#/repos/repo-42';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input);

      if (url === '/api/v1/repos/repo-42') {
        return jsonResponse({
          repository: {
            id: 'repo-42',
            name: 'beta-repo',
            default_branch: 'develop',
            connection_id: 'conn-7',
            sync_state: 'ready',
          },
          connection: {
            id: 'conn-7',
            name: 'GitHub App',
            kind: 'github',
          },
        });
      }

      if (url === '/api/v1/repos/repo-42/tree?path=') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: '',
          entries: [],
        });
      }

      if (url === '/api/v1/repos/repo-42/commits?limit=20') {
        return jsonResponse({
          repo_id: 'repo-42',
          commits: [
            {
              id: 'abc123def456',
              short_id: 'abc123d',
              summary: 'Add commit browser',
              author_name: 'Alice Example',
              authored_at: '2026-04-15T00:00:00Z',
            },
            {
              id: 'fedcba654321',
              short_id: 'fedcba6',
              summary: 'Tidy search UI',
              author_name: 'Bob Example',
              authored_at: '2026-04-14T12:30:00Z',
            },
          ],
        });
      }

      if (url === '/api/v1/repos/repo-42/commits/abc123def456') {
        return jsonResponse({
          repo_id: 'repo-42',
          commit: {
            id: 'abc123def456',
            short_id: 'abc123d',
            summary: 'Add commit browser',
            body: 'Adds a minimal recent commits panel.\n\nIncludes commit metadata.',
            author_name: 'Alice Example',
            authored_at: '2026-04-15T00:00:00Z',
            parents: ['parent-1', 'parent-2'],
          },
        });
      }

      if (url === '/api/v1/repos/repo-42/commits/abc123def456/diff') {
        return jsonResponse({
          repo_id: 'repo-42',
          commit_id: 'abc123def456abc123def456abc123def456abcd',
          files: [
            {
              path: 'web/src/App.tsx',
              change_type: 'modified',
              old_path: null,
              additions: 12,
              deletions: 3,
              patch: '@@ -1,3 +1,4 @@\n-import oldThing\n+import newThing',
            },
            {
              path: 'web/src/NewName.tsx',
              change_type: 'renamed',
              old_path: 'web/src/OldName.tsx',
              additions: 0,
              deletions: 0,
              patch: '@@ -1 +1 @@\n-export * from "./OldName"\n+export * from "./NewName"',
            },
          ],
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Recent commits')).toBeInTheDocument();
    expect(screen.getByText('Add commit browser')).toBeInTheDocument();
    expect(screen.getByText('Alice Example')).toBeInTheDocument();
    expect(screen.getByText('abc123d')).toBeInTheDocument();
    expect(screen.getByText('Select a commit to inspect its details.')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /Add commit browser/i }));

    expect(await screen.findByText('Commit details')).toBeInTheDocument();
    expect(screen.getByText('Adds a minimal recent commits panel. Includes commit metadata.')).toBeInTheDocument();
    expect(screen.getByText('2026-04-15T00:00:00Z')).toBeInTheDocument();
    expect(screen.getByText('parent-1')).toBeInTheDocument();
    expect(screen.getByText('parent-2')).toBeInTheDocument();
    expect(await screen.findByText('Changed files')).toBeInTheDocument();
    expect(screen.getByText('web/src/App.tsx')).toBeInTheDocument();
    expect(screen.getByText('modified')).toBeInTheDocument();
    expect(screen.getByText('+12')).toBeInTheDocument();
    expect(screen.getByText('-3')).toBeInTheDocument();
    expect(screen.getByText('web/src/OldName.tsx → web/src/NewName.tsx')).toBeInTheDocument();
    expect(screen.getByText(/@@ -1,3 \+1,4 @@/)).toBeInTheDocument();
    expect(screen.getByText(/\+import newThing/)).toBeInTheDocument();
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/repos/repo-42/commits?limit=20');
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/repos/repo-42/commits/abc123def456');
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/repos/repo-42/commits/abc123def456/diff');
  });

  it('shows a friendly fallback when a changed file has no patch text', async () => {
    window.location.hash = '#/repos/repo-42';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input);

      if (url === '/api/v1/repos/repo-42') {
        return jsonResponse({
          repository: {
            id: 'repo-42',
            name: 'beta-repo',
            default_branch: 'develop',
            connection_id: 'conn-7',
            sync_state: 'ready',
          },
          connection: {
            id: 'conn-7',
            name: 'GitHub App',
            kind: 'github',
          },
        });
      }

      if (url === '/api/v1/repos/repo-42/tree?path=') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: '',
          entries: [],
        });
      }

      if (url === '/api/v1/repos/repo-42/commits?limit=20') {
        return jsonResponse({
          repo_id: 'repo-42',
          commits: [
            {
              id: 'abc123def456',
              short_id: 'abc123d',
              summary: 'Add binary asset',
              author_name: 'Alice Example',
              authored_at: '2026-04-15T00:00:00Z',
            },
          ],
        });
      }

      if (url === '/api/v1/repos/repo-42/commits/abc123def456') {
        return jsonResponse({
          repo_id: 'repo-42',
          commit: {
            id: 'abc123def456',
            short_id: 'abc123d',
            summary: 'Add binary asset',
            author_name: 'Alice Example',
            authored_at: '2026-04-15T00:00:00Z',
            parents: ['parent-1'],
          },
        });
      }

      if (url === '/api/v1/repos/repo-42/commits/abc123def456/diff') {
        return jsonResponse({
          repo_id: 'repo-42',
          commit_id: 'abc123def456abc123def456abc123def456abcd',
          files: [
            {
              path: 'assets/logo.png',
              change_type: 'added',
              old_path: null,
              additions: 0,
              deletions: 0,
              patch: null,
            },
          ],
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Add binary asset')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /Add binary asset/i }));

    expect(await screen.findByText('assets/logo.png')).toBeInTheDocument();
    expect(screen.getByText('Binary file or patch unavailable.')).toBeInTheDocument();
  });

  it('renders the settings landing page with shared navigation cards', () => {
    window.location.hash = '#/settings';

    render(<App />);

    expect(screen.getByRole('heading', { name: 'Settings' })).toBeInTheDocument();
    expect(screen.getByText('Choose an authenticated admin surface to inspect.')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Connections' })).toHaveAttribute('href', '#/settings/connections');
    expect(screen.getByRole('link', { name: 'API keys' })).toHaveAttribute('href', '#/settings/api-keys');
    expect(screen.getByRole('link', { name: 'Members' })).toHaveAttribute('href', '#/settings/members');
    expect(screen.getByRole('link', { name: 'Access' })).toHaveAttribute('href', '#/settings/access');
    expect(screen.getByRole('link', { name: 'Linked accounts' })).toHaveAttribute('href', '#/settings/linked-accounts');
    expect(screen.getByRole('link', { name: 'OAuth clients' })).toHaveAttribute('href', '#/settings/oauth-clients');
    expect(screen.getByRole('link', { name: 'Audit & analytics' })).toHaveAttribute('href', '#/settings/observability');
    expect(screen.getByRole('link', { name: 'Review automation' })).toHaveAttribute('href', '#/settings/review-automation');
  });

  it('renders linked accounts inventory inside the shared settings shell', async () => {
    window.location.hash = '#/settings/linked-accounts';

    const linkedAccountsResponse = deferredResponse();
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockReturnValueOnce(linkedAccountsResponse.promise);

    render(<App />);

    expect(screen.getByRole('heading', { name: 'Settings' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Linked accounts' })).toBeInTheDocument();
    expect(screen.getByText('Loading linked accounts…')).toBeInTheDocument();

    linkedAccountsResponse.resolve(
      jsonResponse({
        identities: [
          {
            provider: 'local',
            user_id: 'local-user-admin',
            email: 'admin@acme.test',
            name: 'Acme Admin',
            created_at: '2026-04-18T09:00:00Z',
            primary: true,
          },
        ],
        memberships: [
          {
            organization: {
              id: 'org-acme',
              slug: 'acme',
              name: 'Acme, Inc.',
            },
            role: 'admin',
            joined_at: '2026-04-20T09:00:00Z',
          },
          {
            organization: {
              id: 'org-beta',
              slug: 'beta',
              name: 'Beta Org',
            },
            role: 'viewer',
            joined_at: '2026-04-21T09:00:00Z',
          },
        ],
        external_linking_supported: false,
      })
    );

    const localIdentityCard = await screen.findByLabelText('Linked identity local Acme Admin');
    const acmeMembership = screen.getByLabelText('Linked-account membership Acme, Inc.');
    const betaMembership = screen.getByLabelText('Linked-account membership Beta Org');

    expect(fetchMock).toHaveBeenCalledWith('/api/v1/auth/linked-accounts');
    expect(within(localIdentityCard).getByText('User id: local-user-admin')).toBeInTheDocument();
    expect(within(localIdentityCard).getByText('admin@acme.test')).toBeInTheDocument();
    expect(within(localIdentityCard).getByText('Current session identity')).toBeInTheDocument();
    expect(within(acmeMembership).getByText('Organization id: org-acme')).toBeInTheDocument();
    expect(within(acmeMembership).getByText('Slug: acme')).toBeInTheDocument();
    expect(within(acmeMembership).getByText('admin')).toBeInTheDocument();
    expect(within(betaMembership).getByText('viewer')).toBeInTheDocument();
    expect(screen.getByText(/This baseline is intentionally read-only: it shows the current local account identity and visible organization memberships/i)).toBeInTheDocument();
    expect(screen.getByText(/External provider linking and SSO remain follow-up work\./i)).toBeInTheDocument();
  });

  it('shows a linked accounts loading failure inside the shared settings shell', async () => {
    window.location.hash = '#/settings/linked-accounts';

    vi.spyOn(globalThis, 'fetch').mockRejectedValueOnce(new Error('Request failed: 401'));

    render(<App />);

    expect(screen.getByText('Loading linked accounts…')).toBeInTheDocument();
    expect(await screen.findByText('Unable to load linked accounts: Request failed: 401')).toBeInTheDocument();
  });

  it('renders access inventory inside the shared settings shell', async () => {
    window.location.hash = '#/settings/access';

    const reposResponse = deferredResponse();
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockReturnValueOnce(reposResponse.promise);

    render(<App />);

    expect(screen.getByRole('heading', { name: 'Settings' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Access' })).toBeInTheDocument();
    expect(screen.getByText('Loading visible repositories…')).toBeInTheDocument();

    reposResponse.resolve(
      jsonResponse([
        {
          id: 'repo-alpha',
          name: 'alpha-service',
          default_branch: 'main',
          sync_state: 'ready',
        },
        {
          id: 'repo-beta',
          name: 'beta-worker',
          default_branch: 'develop',
          sync_state: 'pending',
        },
      ])
    );

    const alphaRepo = await screen.findByLabelText('Visible repository alpha-service');
    const betaRepo = screen.getByLabelText('Visible repository beta-worker');

    expect(fetchMock).toHaveBeenCalledWith('/api/v1/repos');
    expect(within(alphaRepo).getByText('Repository id: repo-alpha')).toBeInTheDocument();
    expect(within(alphaRepo).getByText('main')).toBeInTheDocument();
    expect(within(alphaRepo).getAllByText('ready')).toHaveLength(2);
    expect(within(betaRepo).getByText('Repository id: repo-beta')).toBeInTheDocument();
    expect(within(betaRepo).getByText('develop')).toBeInTheDocument();
    expect(within(betaRepo).getAllByText('pending')).toHaveLength(2);
    expect(screen.getByText(/The current route is read-only: it lists repositories already visible to your account/i)).toBeInTheDocument();
  });

  it('shows an access empty state inside the shared settings shell', async () => {
    window.location.hash = '#/settings/access';

    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(jsonResponse([]));

    render(<App />);

    expect(screen.getByText('Loading visible repositories…')).toBeInTheDocument();
    expect(await screen.findByText('No visible repositories found')).toBeInTheDocument();
    expect(
      screen.getByText('Your current account does not have any visible repositories yet. Permission sync and access-management workflows remain follow-up work.')
    ).toBeInTheDocument();
  });

  it('renders members inventory inside the shared settings shell', async () => {
    window.location.hash = '#/settings/members';

    const membersResponse = deferredResponse();
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockReturnValueOnce(membersResponse.promise);

    render(<App />);

    expect(screen.getByRole('heading', { name: 'Settings' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Members' })).toBeInTheDocument();
    expect(screen.getByText('Loading members…')).toBeInTheDocument();

    membersResponse.resolve(
      jsonResponse([
        {
          organization: {
            id: 'org-acme',
            slug: 'acme',
            name: 'Acme, Inc.',
          },
          members: [
            {
              user_id: 'local-user-admin',
              role: 'admin',
              joined_at: '2026-04-20T09:00:00Z',
              account: {
                id: 'local-user-admin',
                email: 'admin@acme.test',
                name: 'Acme Admin',
                created_at: '2026-04-18T09:00:00Z',
              },
            },
            {
              user_id: 'local-user-viewer',
              role: 'viewer',
              joined_at: '2026-04-21T09:00:00Z',
              account: {
                id: 'local-user-viewer',
                email: 'viewer@acme.test',
                name: 'Viewer User',
                created_at: '2026-04-19T09:00:00Z',
              },
            },
          ],
          invites: [
            {
              id: 'invite-pending',
              email: 'pending@acme.test',
              role: 'viewer',
              created_at: '2026-04-22T09:00:00Z',
              expires_at: '2026-05-01T09:00:00Z',
              invited_by: {
                id: 'local-user-admin',
                email: 'admin@acme.test',
                name: 'Acme Admin',
              },
              accepted_by: null,
              accepted_at: null,
              status: 'pending',
            },
            {
              id: 'invite-accepted',
              email: 'accepted@acme.test',
              role: 'admin',
              created_at: '2026-04-20T10:00:00Z',
              expires_at: '2026-05-02T10:00:00Z',
              invited_by: {
                id: 'local-user-admin',
                email: 'admin@acme.test',
                name: 'Acme Admin',
              },
              accepted_by: {
                id: 'local-user-viewer',
                email: 'viewer@acme.test',
                name: 'Viewer User',
              },
              accepted_at: '2026-04-21T12:00:00Z',
              status: 'accepted',
            },
          ],
        },
      ])
    );

    const organizationCard = await screen.findByLabelText('Organization members Acme, Inc.');
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/auth/members');
    expect(within(organizationCard).getByText('Organization id: org-acme')).toBeInTheDocument();
    expect(within(organizationCard).getByText('Slug: acme')).toBeInTheDocument();
    expect(within(organizationCard).getByText('Acme Admin')).toBeInTheDocument();
    expect(within(organizationCard).getByText('admin@acme.test')).toBeInTheDocument();
    expect(within(organizationCard).getByText('viewer@acme.test')).toBeInTheDocument();
    expect(within(organizationCard).getByText('pending')).toBeInTheDocument();
    expect(within(organizationCard).getByText('accepted')).toBeInTheDocument();
    expect(within(organizationCard).getByText('Accepted by Viewer User')).toBeInTheDocument();
    expect(within(organizationCard).getAllByText('Invited by Acme Admin')).toHaveLength(2);
  });

  it('shows a members empty state inside the shared settings shell', async () => {
    window.location.hash = '#/settings/members';

    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(jsonResponse([]));

    render(<App />);

    expect(screen.getByText('Loading members…')).toBeInTheDocument();
    expect(await screen.findByText('No administered organizations found')).toBeInTheDocument();
    expect(
      screen.getByText('No member inventory is currently available for your authenticated admin scope.')
    ).toBeInTheDocument();
  });

  it('creates a member invite from the settings members panel after inventory loads', async () => {
    window.location.hash = '#/settings/members';
    window.localStorage.setItem(
      'sourcebot-local-session',
      JSON.stringify({ sessionId: 'session-1', sessionSecret: 'secret-1' })
    );

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/members' && !init?.method) {
        return jsonResponse([
          {
            organization: { id: 'org-acme', slug: 'acme', name: 'Acme, Inc.' },
            members: [],
            invites: [],
          },
        ]);
      }

      if (url === '/api/v1/auth/members/invites' && init?.method === 'POST') {
        expect(init.headers).toMatchObject({
          Authorization: 'Bearer session-1:secret-1',
          'Content-Type': 'application/json',
        });
        expect(init.body).toBe(
          JSON.stringify({ organization_id: 'org-acme', email: 'pending@acme.test', role: 'viewer' })
        );

        return jsonResponse(
          {
            organization: { id: 'org-acme', slug: 'acme', name: 'Acme, Inc.' },
            members: [],
            invites: [
              {
                id: 'invite-created',
                email: 'pending@acme.test',
                role: 'viewer',
                created_at: '2026-04-26T12:00:00Z',
                expires_at: '2026-05-03T12:00:00Z',
                invited_by: { id: 'local-user-admin', email: 'admin@acme.test', name: 'Acme Admin' },
                accepted_by: null,
                accepted_at: null,
                status: 'pending',
              },
            ],
          },
          true,
          201
        );
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    const organizationCard = await screen.findByLabelText('Organization members Acme, Inc.');
    fireEvent.change(within(organizationCard).getByLabelText('Invite email'), {
      target: { value: ' pending@acme.test ' },
    });
    fireEvent.change(within(organizationCard).getByLabelText('Invite role'), { target: { value: 'viewer' } });
    fireEvent.click(within(organizationCard).getByRole('button', { name: 'Create invite for Acme, Inc.' }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/members/invites', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Authorization: 'Bearer session-1:secret-1',
        },
        body: JSON.stringify({ organization_id: 'org-acme', email: 'pending@acme.test', role: 'viewer' }),
      });
    });

    expect(await within(organizationCard).findByText('Invite created for pending@acme.test.')).toBeInTheDocument();
    expect(within(organizationCard).getByText('Invite id: invite-created')).toBeInTheDocument();
    expect(within(organizationCard).getByText('Invited by Acme Admin')).toBeInTheDocument();
    expect(within(organizationCard).getByLabelText('Invite email')).toHaveValue('');
  });

  it('shows a members loading failure inside the shared settings shell', async () => {
    window.location.hash = '#/settings/members';

    vi.spyOn(globalThis, 'fetch').mockRejectedValueOnce(new Error('Request failed: 500'));

    render(<App />);

    expect(screen.getByText('Loading members…')).toBeInTheDocument();
    expect(await screen.findByText('Unable to load members: Request failed: 500')).toBeInTheDocument();
    expect(screen.queryByLabelText('Invite email')).not.toBeInTheDocument();
  });

  it('renders api keys inventory inside the shared settings shell', async () => {
    window.location.hash = '#/settings/api-keys';

    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
      jsonResponse([
        {
          id: 'key-active',
          user_id: 'local-user-1',
          name: 'Personal automation',
          created_at: '2026-04-20T12:00:00Z',
          revoked_at: null,
          repo_scope: [],
        },
        {
          id: 'key-revoked',
          user_id: 'local-user-1',
          name: 'Scoped integration',
          created_at: '2026-04-18T09:30:00Z',
          revoked_at: '2026-04-21T10:00:00Z',
          repo_scope: ['repo-alpha', 'repo-beta'],
        },
      ])
    );

    render(<App />);

    expect(screen.getByRole('heading', { name: 'Settings' })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Connections' })).toHaveAttribute('href', '#/settings/connections');
    expect(screen.getByRole('link', { name: 'API keys' })).toHaveAttribute('href', '#/settings/api-keys');
    expect(screen.getByRole('heading', { name: 'API keys' })).toBeInTheDocument();

    const activeKey = await screen.findByLabelText('API key Personal automation');
    const revokedKey = screen.getByLabelText('API key Scoped integration');

    expect(within(activeKey).getByText('Key id: key-active')).toBeInTheDocument();
    expect(within(activeKey).getByText('local-user-1')).toBeInTheDocument();
    expect(within(activeKey).getByText('2026-04-20T12:00:00Z')).toBeInTheDocument();
    expect(within(activeKey).getByText('active')).toBeInTheDocument();
    expect(
      within(activeKey).getByText('Not repo-bound. This key can reach the repos currently visible to you.')
    ).toBeInTheDocument();

    expect(within(revokedKey).getByText('revoked')).toBeInTheDocument();
    expect(within(revokedKey).getByText('repo-alpha')).toBeInTheDocument();
    expect(within(revokedKey).getByText('repo-beta')).toBeInTheDocument();
    expect(within(revokedKey).getByText('2026-04-21T10:00:00Z')).toBeInTheDocument();
  });

  it('creates an api key, adds it to the inventory, and reveals the plaintext secret only in the immediate success area', async () => {
    window.location.hash = '#/settings/api-keys';
    window.localStorage.setItem(
      'sourcebot-local-session',
      JSON.stringify({ sessionId: 'session-1', sessionSecret: 'secret-1' })
    );

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/api-keys' && !init?.method) {
        return jsonResponse([
          {
            id: 'key-existing',
            user_id: 'local-user-1',
            name: 'Existing automation',
            created_at: '2026-04-20T12:00:00Z',
            revoked_at: null,
            repo_scope: [],
          },
        ]);
      }

      if (url === '/api/v1/auth/api-keys' && init?.method === 'POST') {
        expect(init.headers).toMatchObject({
          Authorization: 'Bearer session-1:secret-1',
          'Content-Type': 'application/json',
        });
        expect(init.body).toBe(JSON.stringify({ name: 'Deploy bot', repo_scope: ['repo-alpha', 'repo-beta'] }));

        return jsonResponse(
          {
            id: 'key-created',
            user_id: 'local-user-1',
            name: 'Deploy bot',
            secret: 'sbp_created_secret_once',
            created_at: '2026-04-26T12:00:00Z',
            revoked_at: null,
            repo_scope: ['repo-alpha', 'repo-beta'],
          },
          true,
          201
        );
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByLabelText('API key Existing automation')).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText('Key name'), { target: { value: ' Deploy bot ' } });
    fireEvent.change(screen.getByLabelText('Repository scope'), { target: { value: 'repo-alpha\n\n repo-beta ' } });
    fireEvent.click(screen.getByRole('button', { name: 'Create API key' }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/api-keys', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Authorization: 'Bearer session-1:secret-1',
        },
        body: JSON.stringify({ name: 'Deploy bot', repo_scope: ['repo-alpha', 'repo-beta'] }),
      });
    });

    const createdKey = await screen.findByLabelText('API key Deploy bot');
    expect(within(createdKey).getByText('Key id: key-created')).toBeInTheDocument();
    expect(within(createdKey).getByText('repo-alpha')).toBeInTheDocument();
    expect(within(createdKey).getByText('repo-beta')).toBeInTheDocument();
    expect(screen.getByText('API key created. Copy the secret now; it will not be shown again.')).toBeInTheDocument();
    expect(screen.getByText('sbp_created_secret_once')).toBeInTheDocument();
    expect(screen.getByLabelText('Key name')).toHaveValue('');
    expect(screen.getByLabelText('Repository scope')).toHaveValue('');
  });

  it('revokes an active api key from the inventory', async () => {
    window.location.hash = '#/settings/api-keys';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/api-keys' && !init) {
        return jsonResponse([
          {
            id: 'key-active',
            user_id: 'local-user-1',
            name: 'Personal automation',
            created_at: '2026-04-20T12:00:00Z',
            revoked_at: null,
            repo_scope: [],
          },
        ]);
      }

      if (url === '/api/v1/auth/api-keys/key-active/revoke' && init?.method === 'POST') {
        return jsonResponse({}, true, 204);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    const activeKey = await screen.findByLabelText('API key Personal automation');
    fireEvent.click(within(activeKey).getByRole('button', { name: 'Revoke key Personal automation' }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/api-keys');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/api-keys/key-active/revoke', { method: 'POST' });
    });

    expect(within(activeKey).getByText('revoked')).toBeInTheDocument();
    expect(
      within(activeKey).getByText('Revoked successfully. Refresh later to load the server timestamp.')
    ).toBeInTheDocument();
    expect(within(activeKey).queryByRole('button', { name: 'Revoke key Personal automation' })).not.toBeInTheDocument();
  });

  it('shows an api keys loading failure without exposing management controls', async () => {
    window.location.hash = '#/settings/api-keys';

    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(jsonResponse({}, false, 503));

    render(<App />);

    expect(screen.getByRole('heading', { name: 'Settings' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'API keys' })).toBeInTheDocument();
    expect(await screen.findByText('Unable to load API keys: Request failed: 503')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Create API key' })).not.toBeInTheDocument();
    expect(screen.queryByLabelText('Key name')).not.toBeInTheDocument();
  });

  it('renders oauth client inventory inside the shared settings shell', async () => {
    window.location.hash = '#/settings/oauth-clients';

    const oauthClientsResponse = deferredResponse();
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockReturnValueOnce(oauthClientsResponse.promise);

    render(<App />);

    expect(screen.getByRole('heading', { name: 'Settings' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'OAuth clients' })).toBeInTheDocument();
    expect(screen.getByText('Loading OAuth clients…')).toBeInTheDocument();

    oauthClientsResponse.resolve(
      jsonResponse([
        {
          id: 'oauth-active',
          organization_id: 'org-acme',
          name: 'Acme Web App',
          client_id: 'client-visible-active',
          created_by_user_id: 'local-user-1',
          created_at: '2026-04-24T00:05:00Z',
          revoked_at: null,
          redirect_uris: ['https://acme.example.com/callback', 'http://localhost:3000/callback'],
          client_secret_hash: 'secret-should-not-render',
        },
        {
          id: 'oauth-revoked',
          organization_id: 'org-acme',
          name: 'Acme CLI',
          client_id: 'client-visible-revoked',
          created_by_user_id: 'local-user-admin',
          created_at: '2026-04-24T00:06:00Z',
          revoked_at: '2026-04-25T00:06:00Z',
          redirect_uris: [],
          client_secret_hash: 'another-secret-that-must-stay-hidden',
        },
      ])
    );

    const activeClient = await screen.findByLabelText('OAuth client Acme Web App');
    const revokedClient = screen.getByLabelText('OAuth client Acme CLI');

    expect(fetchMock).toHaveBeenCalledWith('/api/v1/auth/oauth-clients');
    expect(
      screen.getByText(/Richer OAuth authorization, token issuance and revocation, and broader manage UX remain follow-up work\./)
    ).toBeInTheDocument();

    expect(within(activeClient).getByText('Client id: client-visible-active')).toBeInTheDocument();
    expect(within(activeClient).getByText('org-acme')).toBeInTheDocument();
    expect(within(activeClient).getByText('local-user-1')).toBeInTheDocument();
    expect(within(activeClient).getByText('2026-04-24T00:05:00Z')).toBeInTheDocument();
    expect(within(activeClient).getByText('active')).toBeInTheDocument();
    expect(within(activeClient).getByText('Active')).toBeInTheDocument();
    expect(within(activeClient).getByText('https://acme.example.com/callback')).toBeInTheDocument();
    expect(within(activeClient).getByText('http://localhost:3000/callback')).toBeInTheDocument();

    expect(within(revokedClient).getByText('revoked')).toBeInTheDocument();
    expect(within(revokedClient).getByText('2026-04-25T00:06:00Z')).toBeInTheDocument();
    expect(
      within(revokedClient).getByText('This visible OAuth client does not currently list any redirect URIs.')
    ).toBeInTheDocument();

    expect(screen.queryByText('secret-should-not-render')).not.toBeInTheDocument();
    expect(screen.queryByText('another-secret-that-must-stay-hidden')).not.toBeInTheDocument();
  });

  it('creates an oauth client, refreshes the inventory, and reveals the plaintext secret only in the immediate success area', async () => {
    window.location.hash = '#/settings/oauth-clients';
    window.localStorage.setItem(
      'sourcebot-local-session',
      JSON.stringify({ sessionId: 'session-1', sessionSecret: 'secret-1' })
    );

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/oauth-clients' && !init?.method) {
        return jsonResponse([
          {
            id: 'oauth-existing',
            organization_id: 'org-acme',
            name: 'Existing app',
            client_id: 'client-existing',
            created_by_user_id: 'local-user-1',
            created_at: '2026-04-24T00:05:00Z',
            revoked_at: null,
            redirect_uris: ['https://existing.example.com/callback'],
            client_secret_hash: 'hidden-existing-secret-hash',
          },
        ]);
      }

      if (url === '/api/v1/auth/oauth-clients' && init?.method === 'POST') {
        expect(init.headers).toMatchObject({
          Authorization: 'Bearer session-1:secret-1',
          'Content-Type': 'application/json',
        });
        expect(init.body).toBe(
          JSON.stringify({
            organization_id: 'org-beta',
            name: 'Beta dashboard',
            redirect_uris: ['https://beta.example.com/callback', 'http://localhost:4000/callback'],
          })
        );

        return jsonResponse(
          {
            id: 'oauth-created',
            organization_id: 'org-beta',
            name: 'Beta dashboard',
            client_id: 'client-created',
            client_secret: 'plaintext-secret-once',
            created_by_user_id: 'local-user-admin',
            created_at: '2026-04-26T09:15:00Z',
            revoked_at: null,
            redirect_uris: ['https://beta.example.com/callback', 'http://localhost:4000/callback'],
          },
          true,
          201
        );
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByLabelText('OAuth client Existing app')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Organization id'), { target: { value: ' org-beta ' } });
    fireEvent.change(screen.getByLabelText('Client name'), { target: { value: '  Beta dashboard  ' } });
    fireEvent.change(screen.getByLabelText('Redirect URIs'), {
      target: { value: ' https://beta.example.com/callback \n\n http://localhost:4000/callback ' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Create OAuth client' }));

    expect(await screen.findByText('OAuth client created. Copy the secret now.')).toBeInTheDocument();
    expect(screen.getByText('Client secret: plaintext-secret-once')).toBeInTheDocument();
    expect(screen.getAllByText('Client id: client-created')).toHaveLength(2);

    const createdClient = screen.getByLabelText('OAuth client Beta dashboard');
    expect(within(createdClient).getByText('Client id: client-created')).toBeInTheDocument();
    expect(within(createdClient).queryByText(/plaintext-secret-once/)).not.toBeInTheDocument();

    expect(screen.queryByText('hidden-existing-secret-hash')).not.toBeInTheDocument();
    expect(screen.queryByText('plaintext-secret-once', { selector: 'span' })).not.toBeInTheDocument();

    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/oauth-clients', {
        headers: {
          Authorization: 'Bearer session-1:secret-1',
        },
      });
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/oauth-clients', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Authorization: 'Bearer session-1:secret-1',
        },
        body: JSON.stringify({
          organization_id: 'org-beta',
          name: 'Beta dashboard',
          redirect_uris: ['https://beta.example.com/callback', 'http://localhost:4000/callback'],
        }),
      });
    });
  });

  it('shows an oauth clients empty state', async () => {
    window.location.hash = '#/settings/oauth-clients';

    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(jsonResponse([]));

    render(<App />);

    expect(await screen.findByText('No OAuth clients found')).toBeInTheDocument();
    expect(
      screen.getByText('No visible OAuth clients are currently available for your authenticated organizations.')
    ).toBeInTheDocument();
  });

  it('shows an oauth clients loading failure', async () => {
    window.location.hash = '#/settings/oauth-clients';

    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(jsonResponse({}, false, 503));

    render(<App />);

    expect(await screen.findByText('Unable to load OAuth clients: Request failed: 503')).toBeInTheDocument();
  });

  it('renders audit and analytics inventory inside the shared settings shell', async () => {
    window.location.hash = '#/settings/observability';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/audit-events' && !init) {
        return jsonResponse([
          {
            id: 'audit-visible',
            organization_id: 'org-acme',
            actor: {
              user_id: 'local-user-1',
              api_key_id: 'key-123',
            },
            action: 'auth.api_key.created',
            target_type: 'api_key',
            target_id: 'key-123',
            occurred_at: '2026-04-23T10:15:00Z',
            metadata: {
              name: 'CI automation',
              repo_scope: ['repo-alpha'],
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/analytics' && !init) {
        return jsonResponse([
          {
            id: 'analytics-visible',
            organization_id: 'org-acme',
            metric: 'search.repo.count',
            recorded_at: '2026-04-23T10:20:00Z',
            value: {
              count: 7,
            },
            dimensions: {
              repo_id: 'repo-alpha',
            },
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(screen.getByRole('heading', { name: 'Settings' })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Audit & analytics' })).toHaveAttribute('href', '#/settings/observability');
    expect(screen.getByRole('heading', { name: 'Observability' })).toBeInTheDocument();

    const auditEvent = await screen.findByLabelText('Audit event auth.api_key.created');
    const analyticsRecord = await screen.findByLabelText('Analytics metric search.repo.count');

    expect(within(auditEvent).getByText('org-acme')).toBeInTheDocument();
    expect(within(auditEvent).getByText('key-123')).toBeInTheDocument();
    expect(within(auditEvent).getByText('2026-04-23T10:15:00Z')).toBeInTheDocument();
    expect(within(auditEvent).getByText('{"name":"CI automation","repo_scope":["repo-alpha"]}')).toBeInTheDocument();

    expect(within(analyticsRecord).getByText('org-acme')).toBeInTheDocument();
    expect(within(analyticsRecord).getByText('2026-04-23T10:20:00Z')).toBeInTheDocument();
    expect(within(analyticsRecord).getByText('{"count":7}')).toBeInTheDocument();
    expect(within(analyticsRecord).getByText('{"repo_id":"repo-alpha"}')).toBeInTheDocument();

    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/audit-events');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/analytics');
    });
  });

  it('shows observability loading failures per endpoint', async () => {
    window.location.hash = '#/settings/observability';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/audit-events' && !init) {
        return jsonResponse({}, false, 503);
      }

      if (url === '/api/v1/auth/analytics' && !init) {
        return jsonResponse({}, false, 502);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(screen.getByRole('heading', { name: 'Observability' })).toBeInTheDocument();
    expect(await screen.findByText('Unable to load audit events: Request failed: 503')).toBeInTheDocument();
    expect(screen.getByText('Unable to load analytics: Request failed: 502')).toBeInTheDocument();
  });

  it('renders dedicated agents route with restored run detail and related resources', async () => {
    window.location.hash = '#/agents?run_id=run-visible';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/review-agent-runs' && !init) {
        return jsonResponse([
          {
            id: 'run-visible',
            organization_id: 'org-acme',
            webhook_id: 'review-webhook-visible',
            delivery_attempt_id: 'delivery-visible',
            connection_id: 'conn-github',
            repository_id: 'repo-alpha',
            review_id: 'review-42',
            status: 'failed',
            created_at: '2026-04-25T09:02:00Z',
          },
        ]);
      }

      if (url === '/api/v1/auth/review-agent-runs/run-visible' && !init) {
        return jsonResponse({
          id: 'run-visible',
          organization_id: 'org-acme',
          webhook_id: 'review-webhook-visible',
          delivery_attempt_id: 'delivery-visible',
          connection_id: 'conn-github',
          repository_id: 'repo-alpha',
          review_id: 'review-42',
          status: 'failed',
          created_at: '2026-04-25T09:02:00Z',
        });
      }

      if (url === '/api/v1/auth/review-webhook-delivery-attempts/delivery-visible' && !init) {
        return jsonResponse({
          id: 'delivery-visible',
          webhook_id: 'review-webhook-visible',
          connection_id: 'conn-github',
          repository_id: 'repo-alpha',
          event_type: 'pull_request.opened',
          review_id: 'review-42',
          external_event_id: 'evt-99',
          accepted_at: '2026-04-25T09:01:00Z',
        });
      }

      if (url === '/api/v1/auth/review-webhooks/review-webhook-visible' && !init) {
        return jsonResponse({
          id: 'review-webhook-visible',
          organization_id: 'org-acme',
          connection_id: 'conn-github',
          repository_id: 'repo-alpha',
          events: ['pull_request.opened', 'pull_request.synchronize'],
          created_by_user_id: 'local-user-1',
          created_at: '2026-04-25T09:00:00Z',
          secret_hash: 'must-not-render',
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(screen.getByRole('link', { name: 'Agents' })).toHaveAttribute('href', '#/agents');
    expect(screen.getByRole('heading', { name: 'Agents' })).toBeInTheDocument();
    expect(screen.getByText('Inspect visible review-agent runs from a dedicated operator route.')).toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'Settings' })).not.toBeInTheDocument();

    const run = await screen.findByLabelText('Agent run run-visible');
    const attempt = await screen.findByLabelText('Selected delivery attempt delivery-visible');
    const webhook = await screen.findByLabelText('Selected review webhook review-webhook-visible');

    expect(within(run).getByText('failed')).toBeInTheDocument();
    expect(within(run).getByText('org-acme')).toBeInTheDocument();
    expect(within(run).getByText('review-webhook-visible')).toBeInTheDocument();
    expect(within(run).getByText('delivery-visible')).toBeInTheDocument();
    expect(within(run).getByText('2026-04-25T09:02:00Z')).toBeInTheDocument();
    expect(within(run).getByText('Selected run')).toBeInTheDocument();

    expect(within(attempt).getByText('Event type: pull_request.opened')).toBeInTheDocument();
    expect(within(attempt).getByText('review-42')).toBeInTheDocument();
    expect(within(attempt).getByText('evt-99')).toBeInTheDocument();
    expect(within(attempt).getByText('2026-04-25T09:01:00Z')).toBeInTheDocument();

    expect(within(webhook).getByText('repo-alpha')).toBeInTheDocument();
    expect(within(webhook).getByText('conn-github')).toBeInTheDocument();
    expect(within(webhook).getByText('local-user-1')).toBeInTheDocument();
    expect(within(webhook).getByText('2026-04-25T09:00:00Z')).toBeInTheDocument();
    expect(within(webhook).getByText('pull_request.opened')).toBeInTheDocument();
    expect(within(webhook).getByText('pull_request.synchronize')).toBeInTheDocument();

    expect(screen.queryByText('must-not-render')).not.toBeInTheDocument();
    expect(window.location.hash).toBe('#/agents?run_id=run-visible');

    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/review-agent-runs');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/review-agent-runs/run-visible');
      expect(fetchMock).toHaveBeenNthCalledWith(3, '/api/v1/auth/review-webhook-delivery-attempts/delivery-visible');
      expect(fetchMock).toHaveBeenNthCalledWith(4, '/api/v1/auth/review-webhooks/review-webhook-visible');
    });
  });

  it('reloads the selected agent run when re-opening the active run details', async () => {
    window.location.hash = '#/agents?run_id=run-visible';

    const runDetailResponses = [
      {
        id: 'run-visible',
        organization_id: 'org-acme',
        webhook_id: 'review-webhook-visible',
        delivery_attempt_id: 'delivery-visible',
        connection_id: 'conn-github',
        repository_id: 'repo-alpha',
        review_id: 'review-42',
        status: 'queued',
        created_at: '2026-04-25T09:02:00Z',
      },
      {
        id: 'run-visible',
        organization_id: 'org-acme',
        webhook_id: 'review-webhook-visible',
        delivery_attempt_id: 'delivery-visible',
        connection_id: 'conn-github',
        repository_id: 'repo-alpha',
        review_id: 'review-42',
        status: 'completed',
        created_at: '2026-04-25T09:03:00Z',
      },
    ];

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/review-agent-runs' && !init) {
        return jsonResponse([
          {
            id: 'run-visible',
            organization_id: 'org-acme',
            webhook_id: 'review-webhook-visible',
            delivery_attempt_id: 'delivery-visible',
            connection_id: 'conn-github',
            repository_id: 'repo-alpha',
            review_id: 'review-42',
            status: 'queued',
            created_at: '2026-04-25T09:02:00Z',
          },
        ]);
      }

      if (url === '/api/v1/auth/review-agent-runs/run-visible' && !init) {
        const next = runDetailResponses.shift();
        if (!next) {
          throw new Error('No run detail response left');
        }
        return jsonResponse(next);
      }

      if (url === '/api/v1/auth/review-webhook-delivery-attempts/delivery-visible' && !init) {
        return jsonResponse({
          id: 'delivery-visible',
          webhook_id: 'review-webhook-visible',
          connection_id: 'conn-github',
          repository_id: 'repo-alpha',
          event_type: 'pull_request.opened',
          review_id: 'review-42',
          external_event_id: 'evt-99',
          accepted_at: '2026-04-25T09:01:00Z',
        });
      }

      if (url === '/api/v1/auth/review-webhooks/review-webhook-visible' && !init) {
        return jsonResponse({
          id: 'review-webhook-visible',
          organization_id: 'org-acme',
          connection_id: 'conn-github',
          repository_id: 'repo-alpha',
          events: ['pull_request.opened'],
          created_by_user_id: 'local-user-1',
          created_at: '2026-04-25T09:00:00Z',
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    const selectedRunCard = await screen.findByLabelText('Selected agent run run-visible');
    expect(within(selectedRunCard).getByText('queued')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Reload details' }));

    await waitFor(() => {
      expect(fetchMock.mock.calls.filter(([url]) => String(url) === '/api/v1/auth/review-agent-runs/run-visible')).toHaveLength(2);
    });
    expect(within(await screen.findByLabelText('Selected agent run run-visible')).getByText('completed')).toBeInTheDocument();
    expect(screen.getByText('2026-04-25T09:03:00Z')).toBeInTheDocument();
  });

  it('keeps webhook visibility when the selected delivery attempt fails to load', async () => {
    window.location.hash = '#/agents?run_id=run-visible';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/review-agent-runs' && !init) {
        return jsonResponse([
          {
            id: 'run-visible',
            organization_id: 'org-acme',
            webhook_id: 'review-webhook-visible',
            delivery_attempt_id: 'delivery-visible',
            connection_id: 'conn-github',
            repository_id: 'repo-alpha',
            review_id: 'review-42',
            status: 'failed',
            created_at: '2026-04-25T09:02:00Z',
          },
        ]);
      }

      if (url === '/api/v1/auth/review-agent-runs/run-visible' && !init) {
        return jsonResponse({
          id: 'run-visible',
          organization_id: 'org-acme',
          webhook_id: 'review-webhook-visible',
          delivery_attempt_id: 'delivery-visible',
          connection_id: 'conn-github',
          repository_id: 'repo-alpha',
          review_id: 'review-42',
          status: 'failed',
          created_at: '2026-04-25T09:02:00Z',
        });
      }

      if (url === '/api/v1/auth/review-webhook-delivery-attempts/delivery-visible' && !init) {
        return jsonResponse({}, false, 502);
      }

      if (url === '/api/v1/auth/review-webhooks/review-webhook-visible' && !init) {
        return jsonResponse({
          id: 'review-webhook-visible',
          organization_id: 'org-acme',
          connection_id: 'conn-github',
          repository_id: 'repo-alpha',
          events: ['pull_request.opened', 'pull_request.synchronize'],
          created_by_user_id: 'local-user-1',
          created_at: '2026-04-25T09:00:00Z',
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Unable to load selected delivery attempt: Request failed: 502')).toBeInTheDocument();
    const webhook = await screen.findByLabelText('Selected review webhook review-webhook-visible');
    expect(within(webhook).getByText('repo-alpha')).toBeInTheDocument();
    expect(within(webhook).getByText('pull_request.synchronize')).toBeInTheDocument();

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith('/api/v1/auth/review-webhooks/review-webhook-visible');
    });
  });

  it('fails closed for a missing restored run on the dedicated agents route', async () => {
    window.location.hash = '#/agents?run_id=run-missing';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/review-agent-runs' && !init) {
        return jsonResponse([
          {
            id: 'run-visible',
            organization_id: 'org-acme',
            webhook_id: 'review-webhook-visible',
            delivery_attempt_id: 'delivery-visible',
            connection_id: 'conn-github',
            repository_id: 'repo-alpha',
            review_id: 'review-42',
            status: 'queued',
            created_at: '2026-04-25T09:02:00Z',
          },
        ]);
      }

      if (url === '/api/v1/auth/review-agent-runs/run-missing' && !init) {
        return jsonResponse({ error: 'missing' }, false, 404);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('The restored agent run is no longer visible.')).toBeInTheDocument();
    expect(screen.queryByLabelText('Selected agent run run-missing')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('Selected delivery attempt delivery-visible')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('Selected review webhook review-webhook-visible')).not.toBeInTheDocument();
    expect(window.location.hash).toBe('#/agents');

    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/review-agent-runs');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/review-agent-runs/run-missing');
    });
  });

  it('renders review automation inventory inside the shared settings shell', async () => {
    window.location.hash = '#/settings/review-automation';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/review-webhooks' && !init) {
        return jsonResponse([
          {
            id: 'review-webhook-visible',
            organization_id: 'org-acme',
            connection_id: 'conn-github',
            repository_id: 'repo-alpha',
            events: ['pull_request.opened', 'pull_request.synchronize'],
            created_by_user_id: 'local-user-1',
            created_at: '2026-04-25T09:00:00Z',
            secret_hash: 'must-not-render',
          },
        ]);
      }

      if (url === '/api/v1/auth/review-webhook-delivery-attempts' && !init) {
        return jsonResponse([
          {
            id: 'delivery-visible',
            webhook_id: 'review-webhook-visible',
            connection_id: 'conn-github',
            repository_id: 'repo-alpha',
            event_type: 'pull_request.opened',
            review_id: 'review-42',
            external_event_id: 'evt-99',
            accepted_at: '2026-04-25T09:01:00Z',
          },
        ]);
      }

      if (url === '/api/v1/auth/review-agent-runs' && !init) {
        return jsonResponse([
          {
            id: 'run-visible',
            organization_id: 'org-acme',
            webhook_id: 'review-webhook-visible',
            delivery_attempt_id: 'delivery-visible',
            connection_id: 'conn-github',
            repository_id: 'repo-alpha',
            review_id: 'review-42',
            status: 'failed',
            created_at: '2026-04-25T09:02:00Z',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(screen.getByRole('heading', { name: 'Settings' })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Review automation' })).toHaveAttribute('href', '#/settings/review-automation');
    expect(screen.getByRole('heading', { name: 'Review automation' })).toBeInTheDocument();

    const webhook = await screen.findByLabelText('Review webhook review-webhook-visible');
    const attempt = await screen.findByLabelText('Delivery attempt delivery-visible');
    const run = await screen.findByLabelText('Review-agent run run-visible');

    expect(within(webhook).getByText('repo-alpha')).toBeInTheDocument();
    expect(within(webhook).getByText('conn-github')).toBeInTheDocument();
    expect(within(webhook).getByText('local-user-1')).toBeInTheDocument();
    expect(within(webhook).getByText('2026-04-25T09:00:00Z')).toBeInTheDocument();
    expect(within(webhook).getByText('pull_request.opened')).toBeInTheDocument();
    expect(within(webhook).getByText('pull_request.synchronize')).toBeInTheDocument();

    expect(within(attempt).getByText('repo-alpha')).toBeInTheDocument();
    expect(within(attempt).getByText('review-42')).toBeInTheDocument();
    expect(within(attempt).getByText('evt-99')).toBeInTheDocument();
    expect(within(attempt).getByText('2026-04-25T09:01:00Z')).toBeInTheDocument();

    expect(within(run).getByText('failed')).toBeInTheDocument();
    expect(within(run).getByText('org-acme')).toBeInTheDocument();
    expect(within(run).getByText('review-webhook-visible')).toBeInTheDocument();
    expect(within(run).getByText('delivery-visible')).toBeInTheDocument();
    expect(within(run).getByText('2026-04-25T09:02:00Z')).toBeInTheDocument();

    expect(screen.queryByText('must-not-render')).not.toBeInTheDocument();

    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/review-webhooks');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/review-webhook-delivery-attempts');
      expect(fetchMock).toHaveBeenNthCalledWith(3, '/api/v1/auth/review-agent-runs');
    });
  });

  it('shows review automation loading failures per endpoint', async () => {
    window.location.hash = '#/settings/review-automation';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/review-webhooks' && !init) {
        return jsonResponse({}, false, 503);
      }

      if (url === '/api/v1/auth/review-webhook-delivery-attempts' && !init) {
        return jsonResponse({}, false, 502);
      }

      if (url === '/api/v1/auth/review-agent-runs' && !init) {
        return jsonResponse({}, false, 500);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(screen.getByRole('heading', { name: 'Review automation' })).toBeInTheDocument();
    expect(await screen.findByText('Unable to load review webhooks: Request failed: 503')).toBeInTheDocument();
    expect(screen.getByText('Unable to load delivery attempts: Request failed: 502')).toBeInTheDocument();
    expect(screen.getByText('Unable to load review-agent runs: Request failed: 500')).toBeInTheDocument();
  });

  it('renders the Authenticated connections route inside the shared settings shell', async () => {
    window.location.hash = '#/settings/connections';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'Local Mirror',
            kind: 'local',
            config: {
              provider: 'local',
              repo_path: '/srv/git/mirror',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/connections' && init?.method === 'POST') {
        expect(init.headers).toEqual({ 'Content-Type': 'application/json' });
        expect(init.body).toBe(
          JSON.stringify({
            name: ' Acme GitHub ',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.example.com',
            },
          })
        );

        return jsonResponse(
          {
            id: 'conn-3',
            name: 'Acme GitHub',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.example.com',
            },
          },
          true,
          201
        );
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(screen.getByRole('heading', { name: 'Settings' })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Connections' })).toHaveAttribute('href', '#/settings/connections');
    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    expect(screen.getByText('GitHub Cloud')).toBeInTheDocument();
    expect(screen.getByText('Kind: github')).toBeInTheDocument();
    expect(screen.getByText('Base URL: https://github.com')).toBeInTheDocument();
    expect(screen.getByText('Local Mirror')).toBeInTheDocument();
    expect(screen.getByText('Repo path: /srv/git/mirror')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Connection name'), { target: { value: ' Acme GitHub ' } });
    fireEvent.change(screen.getByLabelText('Connection kind'), { target: { value: 'github' } });
    fireEvent.change(screen.getByLabelText('Base URL'), { target: { value: 'https://github.example.com' } });
    fireEvent.click(screen.getByRole('button', { name: 'Create connection' }));

    expect(await screen.findByText('Acme GitHub')).toBeInTheDocument();
    expect(screen.getByText('Base URL: https://github.example.com')).toBeInTheDocument();
    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/connections');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/repository-sync-jobs');
      expect(fetchMock).toHaveBeenNthCalledWith(3, '/api/v1/auth/connections', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name: ' Acme GitHub ',
          kind: 'github',
          config: {
            provider: 'github',
            base_url: 'https://github.example.com',
          },
        }),
      });
    });
  });

  it('renders the authenticated connections inventory and creates a new generic git connection from the settings route using the host-style base_url contract', async () => {
    window.location.hash = '#/settings/connections';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'Local Mirror',
            kind: 'local',
            config: {
              provider: 'local',
              repo_path: '/srv/git/mirror',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/connections' && init?.method === 'POST') {
        expect(init.headers).toEqual({ 'Content-Type': 'application/json' });
        expect(init.body).toBe(
          JSON.stringify({
            name: ' Generic Git Host ',
            kind: 'generic_git',
            config: {
              provider: 'generic_git',
              base_url: 'https://git.internal.example.com',
            },
          })
        );

        return jsonResponse(
          {
            id: 'conn-3',
            name: 'Generic Git Host',
            kind: 'generic_git',
            config: {
              provider: 'generic_git',
              base_url: 'https://git.internal.example.com',
            },
          },
          true,
          201
        );
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    expect(screen.getByText('GitHub Cloud')).toBeInTheDocument();
    expect(screen.getByText('Base URL: https://github.com')).toBeInTheDocument();
    expect(screen.getByText('Local Mirror')).toBeInTheDocument();
    expect(screen.getByText('Repo path: /srv/git/mirror')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Connection name'), { target: { value: ' Generic Git Host ' } });
    fireEvent.change(screen.getByLabelText('Connection kind'), { target: { value: 'generic_git' } });
    fireEvent.change(screen.getByLabelText('Base URL'), { target: { value: 'https://git.internal.example.com' } });
    fireEvent.click(screen.getByRole('button', { name: 'Create connection' }));

    expect(await screen.findByText('Generic Git Host')).toBeInTheDocument();
    expect(screen.getByText('Base URL: https://git.internal.example.com')).toBeInTheDocument();
    expect(screen.getByText('Kind: generic_git')).toBeInTheDocument();
    expect(screen.getByText('Connection id: conn-3')).toBeInTheDocument();
    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/connections');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/repository-sync-jobs');
      expect(fetchMock).toHaveBeenNthCalledWith(3, '/api/v1/auth/connections', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name: ' Generic Git Host ',
          kind: 'generic_git',
          config: {
            provider: 'generic_git',
            base_url: 'https://git.internal.example.com',
          },
        }),
      });
    });
  });

  it('shows read-only repository sync-job history on each authenticated connection card from the settings route', async () => {
    window.location.hash = '#/settings/connections';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'Local Mirror',
            kind: 'local',
            config: {
              provider: 'local',
              repo_path: '/srv/git/mirror',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-1',
            organization_id: 'org-1',
            repository_id: 'repo-123',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T10:00:00Z',
            started_at: '2026-04-18T10:01:00Z',
            finished_at: '2026-04-18T10:02:00Z',
            error: null,
          },
          {
            id: 'job-2',
            organization_id: 'org-1',
            repository_id: 'repo-456',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:01:00Z',
            finished_at: '2026-04-18T11:03:00Z',
            error: 'Repository permissions denied',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    expect(await screen.findAllByText('Repository sync history')).toHaveLength(2);
    expect(screen.getByText('Repository id: repo-123')).toBeInTheDocument();
    const succeededSyncRow = screen.getByRole('link', { name: 'Open repository detail for repo-123' }).closest('div');
    expect(succeededSyncRow).toBeInTheDocument();
    const succeededStatusRow = within(succeededSyncRow!).getByText('Status').closest('div');
    expect(succeededStatusRow).toBeInTheDocument();
    expect(within(succeededStatusRow!).getByText('succeeded')).toBeInTheDocument();
    expect(within(succeededSyncRow!).queryByText('Status: succeeded')).not.toBeInTheDocument();
    expect(screen.getByText('Queued at: 2026-04-18T10:00:00Z')).toBeInTheDocument();
    expect(screen.getByText('Started at: 2026-04-18T10:01:00Z')).toBeInTheDocument();
    expect(screen.getByText('Finished at: 2026-04-18T10:02:00Z')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Open repository detail for repo-123' })).toHaveAttribute(
      'href',
      '#/repos/repo-123'
    );
    const failedSyncRow = screen.getByRole('link', { name: 'Open repository detail for repo-456' }).closest('div');
    expect(failedSyncRow).toBeInTheDocument();
    const failedStatusRow = within(failedSyncRow!).getByText('Status').closest('div');
    expect(failedStatusRow).toBeInTheDocument();
    expect(within(failedStatusRow!).getByText('failed')).toBeInTheDocument();
    expect(within(failedSyncRow!).queryByText('Status: failed')).not.toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Open repository detail for repo-456' })).toHaveAttribute(
      'href',
      '#/repos/repo-456'
    );
    expect(screen.getByText('Error: Repository permissions denied')).toBeInTheDocument();
    expect(screen.getByText('No repository sync jobs found for this connection.')).toBeInTheDocument();
    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/connections');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/repository-sync-jobs');
    });
  });

  it('renders each connection\'s repository sync history newest-first when sync jobs arrive out of order', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-older',
            organization_id: 'org-1',
            repository_id: 'repo-older',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T10:00:00Z',
            started_at: '2026-04-18T10:01:00Z',
            finished_at: '2026-04-18T10:03:00Z',
            error: 'Older failure',
          },
          {
            id: 'job-other-connection',
            organization_id: 'org-1',
            repository_id: 'repo-other-connection',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T11:30:00Z',
            started_at: '2026-04-18T11:31:00Z',
            finished_at: '2026-04-18T11:33:00Z',
            error: null,
          },
          {
            id: 'job-newer',
            organization_id: 'org-1',
            repository_id: 'repo-newer',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:01:00Z',
            finished_at: '2026-04-18T12:02:00Z',
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();
    const repositoryIds = within(githubCard!).getAllByText(/Repository id:/).map((node) => node.textContent);
    const queuedTimestamps = within(githubCard!).getAllByText(/Queued at:/).map((node) => node.textContent);

    expect(repositoryIds).toEqual(['Repository id: repo-newer', 'Repository id: repo-older']);
    expect(queuedTimestamps).toEqual(['Queued at: 2026-04-18T12:00:00Z', 'Queued at: 2026-04-18T10:00:00Z']);

    const githubLatestSyncSummary = within(githubCard!).getByText(/Latest sync:/).closest('div');
    expect(githubLatestSyncSummary).toBeInTheDocument();
    expect(within(githubLatestSyncSummary!).getByText('succeeded')).toBeInTheDocument();
    expect(within(githubLatestSyncSummary!).getByText('repo-newer · 2026-04-18T12:00:00Z')).toBeInTheDocument();
    expect(within(githubCard!).getByText('Error: Older failure')).toBeInTheDocument();

    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(gitlabCard).toBeInTheDocument();
    const gitlabLatestSyncSummary = within(gitlabCard!).getByText(/Latest sync:/).closest('div');
    expect(gitlabLatestSyncSummary).toBeInTheDocument();
    expect(within(gitlabLatestSyncSummary!).getByText('succeeded')).toBeInTheDocument();
    expect(within(gitlabLatestSyncSummary!).getByText('repo-other-connection · 2026-04-18T11:30:00Z')).toBeInTheDocument();
    expect(within(gitlabCard!).getByText('Repository id: repo-other-connection')).toBeInTheDocument();
  });

  it('keeps terminal-state sync-history rows scoped to their sibling authenticated connection cards on the settings route', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-failed-newest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-failed-newest',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:03:00Z',
            error: 'GitHub permissions denied',
          },
          {
            id: 'job-conn-2-succeeded-newest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-succeeded-newest',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T12:30:00Z',
            started_at: '2026-04-18T12:31:00Z',
            finished_at: '2026-04-18T12:33:00Z',
            error: null,
          },
          {
            id: 'job-conn-1-succeeded-older',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-succeeded-older',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:01:00Z',
            finished_at: '2026-04-18T11:02:00Z',
            error: null,
          },
          {
            id: 'job-conn-2-failed-older',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-failed-older',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T10:00:00Z',
            started_at: '2026-04-18T10:01:00Z',
            finished_at: '2026-04-18T10:04:00Z',
            error: 'GitLab import failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-conn-1-failed-newest',
      '#/repos/repo-conn-1-succeeded-older',
    ]);
    expect(within(githubCard!).queryByText('Repository id: repo-conn-2-succeeded-newest')).not.toBeInTheDocument();
    expect(within(githubCard!).queryByText('Repository id: repo-conn-2-failed-older')).not.toBeInTheDocument();
    const githubLatestSyncSummary = within(githubCard!).getByText(/Latest sync:/).closest('div');
    expect(githubLatestSyncSummary).toBeInTheDocument();
    expect(within(githubLatestSyncSummary!).getByText('failed')).toBeInTheDocument();
    expect(within(githubLatestSyncSummary!).getByText('repo-conn-1-failed-newest · 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(githubCard!).getByText('Error: GitHub permissions denied')).toBeInTheDocument();
    expect(within(githubCard!).queryByText('Error: GitLab import failed')).not.toBeInTheDocument();

    expect(within(gitlabCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-conn-2-succeeded-newest',
      '#/repos/repo-conn-2-failed-older',
    ]);
    expect(within(gitlabCard!).queryByText('Repository id: repo-conn-1-failed-newest')).not.toBeInTheDocument();
    expect(within(gitlabCard!).queryByText('Repository id: repo-conn-1-succeeded-older')).not.toBeInTheDocument();
    const gitlabLatestSyncSummary = within(gitlabCard!).getByText(/Latest sync:/).closest('div');
    expect(gitlabLatestSyncSummary).toBeInTheDocument();
    expect(within(gitlabLatestSyncSummary!).getByText('succeeded')).toBeInTheDocument();
    expect(within(gitlabLatestSyncSummary!).getByText('repo-conn-2-succeeded-newest · 2026-04-18T12:30:00Z')).toBeInTheDocument();
    expect(within(gitlabCard!).getByText('Error: GitLab import failed')).toBeInTheDocument();
    expect(within(gitlabCard!).queryByText('Error: GitHub permissions denied')).not.toBeInTheDocument();
  });

  it('keeps multiple succeeded and failed terminal-state sync-history rows newest-first by queued_at within each sibling authenticated connection card on the settings route', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-succeeded-middle',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-succeeded-middle',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:01:00Z',
            finished_at: '2026-04-18T12:05:00Z',
            error: null,
          },
          {
            id: 'job-conn-2-failed-oldest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-failed-oldest',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T10:00:00Z',
            started_at: '2026-04-18T10:01:00Z',
            finished_at: '2026-04-18T10:04:00Z',
            error: 'GitLab import failed',
          },
          {
            id: 'job-conn-1-failed-newest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-failed-newest',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:03:00Z',
            error: 'GitHub permissions denied',
          },
          {
            id: 'job-conn-2-succeeded-newest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-succeeded-newest',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T12:30:00Z',
            started_at: '2026-04-18T12:31:00Z',
            finished_at: '2026-04-18T12:33:00Z',
            error: null,
          },
          {
            id: 'job-conn-1-succeeded-oldest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-succeeded-oldest',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:01:00Z',
            finished_at: '2026-04-18T11:02:00Z',
            error: null,
          },
          {
            id: 'job-conn-2-failed-middle',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-failed-middle',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T11:30:00Z',
            started_at: '2026-04-18T11:31:00Z',
            finished_at: '2026-04-18T11:34:00Z',
            error: 'GitLab permissions denied',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-conn-1-failed-newest',
      '#/repos/repo-conn-1-succeeded-middle',
      '#/repos/repo-conn-1-succeeded-oldest',
    ]);
    expect(within(githubCard!).getAllByText(/Queued at:/).map((node) => node.textContent)).toEqual([
      'Queued at: 2026-04-18T13:00:00Z',
      'Queued at: 2026-04-18T12:00:00Z',
      'Queued at: 2026-04-18T11:00:00Z',
    ]);

    expect(within(gitlabCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-conn-2-succeeded-newest',
      '#/repos/repo-conn-2-failed-middle',
      '#/repos/repo-conn-2-failed-oldest',
    ]);
    expect(within(gitlabCard!).getAllByText(/Queued at:/).map((node) => node.textContent)).toEqual([
      'Queued at: 2026-04-18T12:30:00Z',
      'Queued at: 2026-04-18T11:30:00Z',
      'Queued at: 2026-04-18T10:00:00Z',
    ]);
  });

  it('keeps sibling terminal-state sync-history rows using the shared status badge presentation truthful within each authenticated connection card when multiple rows are present', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-succeeded-middle',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-succeeded-middle',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:01:00Z',
            finished_at: '2026-04-18T12:05:00Z',
            error: null,
          },
          {
            id: 'job-conn-2-failed-oldest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-failed-oldest',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T10:00:00Z',
            started_at: '2026-04-18T10:01:00Z',
            finished_at: '2026-04-18T10:04:00Z',
            error: 'GitLab import failed',
          },
          {
            id: 'job-conn-1-failed-newest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-failed-newest',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:03:00Z',
            error: 'GitHub permissions denied',
          },
          {
            id: 'job-conn-2-succeeded-newest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-succeeded-newest',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T12:30:00Z',
            started_at: '2026-04-18T12:31:00Z',
            finished_at: '2026-04-18T12:33:00Z',
            error: null,
          },
          {
            id: 'job-conn-1-succeeded-oldest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-succeeded-oldest',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:01:00Z',
            finished_at: '2026-04-18T11:02:00Z',
            error: null,
          },
          {
            id: 'job-conn-2-failed-middle',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-failed-middle',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T11:30:00Z',
            started_at: '2026-04-18T11:31:00Z',
            finished_at: '2026-04-18T11:34:00Z',
            error: 'GitLab permissions denied',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubFailedRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-conn-1-failed-newest' }).closest('div');
    expect(githubFailedRow).toBeInTheDocument();
    const githubFailedStatusRow = within(githubFailedRow!).getByText('Status').closest('div');
    expect(githubFailedStatusRow).toBeInTheDocument();
    expect(within(githubFailedStatusRow!).getByText('failed')).toBeInTheDocument();
    expect(within(githubFailedRow!).queryByText('Status: failed')).not.toBeInTheDocument();
    expect(within(githubFailedRow!).queryByText('succeeded')).not.toBeInTheDocument();

    const githubSucceededMiddleRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-conn-1-succeeded-middle' }).closest('div');
    expect(githubSucceededMiddleRow).toBeInTheDocument();
    const githubSucceededMiddleStatusRow = within(githubSucceededMiddleRow!).getByText('Status').closest('div');
    expect(githubSucceededMiddleStatusRow).toBeInTheDocument();
    expect(within(githubSucceededMiddleStatusRow!).getByText('succeeded')).toBeInTheDocument();
    expect(within(githubSucceededMiddleRow!).queryByText('Status: succeeded')).not.toBeInTheDocument();
    expect(within(githubSucceededMiddleRow!).queryByText('failed')).not.toBeInTheDocument();

    const githubSucceededOldestRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-conn-1-succeeded-oldest' }).closest('div');
    expect(githubSucceededOldestRow).toBeInTheDocument();
    const githubSucceededOldestStatusRow = within(githubSucceededOldestRow!).getByText('Status').closest('div');
    expect(githubSucceededOldestStatusRow).toBeInTheDocument();
    expect(within(githubSucceededOldestStatusRow!).getByText('succeeded')).toBeInTheDocument();
    expect(within(githubSucceededOldestRow!).queryByText('Status: succeeded')).not.toBeInTheDocument();
    expect(within(githubSucceededOldestRow!).queryByText('failed')).not.toBeInTheDocument();

    const gitlabSucceededRow = within(gitlabCard!).getByRole('link', { name: 'Open repository detail for repo-conn-2-succeeded-newest' }).closest('div');
    expect(gitlabSucceededRow).toBeInTheDocument();
    const gitlabSucceededStatusRow = within(gitlabSucceededRow!).getByText('Status').closest('div');
    expect(gitlabSucceededStatusRow).toBeInTheDocument();
    expect(within(gitlabSucceededStatusRow!).getByText('succeeded')).toBeInTheDocument();
    expect(within(gitlabSucceededRow!).queryByText('Status: succeeded')).not.toBeInTheDocument();
    expect(within(gitlabSucceededRow!).queryByText('failed')).not.toBeInTheDocument();

    const gitlabFailedMiddleRow = within(gitlabCard!).getByRole('link', { name: 'Open repository detail for repo-conn-2-failed-middle' }).closest('div');
    expect(gitlabFailedMiddleRow).toBeInTheDocument();
    const gitlabFailedMiddleStatusRow = within(gitlabFailedMiddleRow!).getByText('Status').closest('div');
    expect(gitlabFailedMiddleStatusRow).toBeInTheDocument();
    expect(within(gitlabFailedMiddleStatusRow!).getByText('failed')).toBeInTheDocument();
    expect(within(gitlabFailedMiddleRow!).queryByText('Status: failed')).not.toBeInTheDocument();
    expect(within(gitlabFailedMiddleRow!).queryByText('succeeded')).not.toBeInTheDocument();

    const gitlabFailedOldestRow = within(gitlabCard!).getByRole('link', { name: 'Open repository detail for repo-conn-2-failed-oldest' }).closest('div');
    expect(gitlabFailedOldestRow).toBeInTheDocument();
    const gitlabFailedOldestStatusRow = within(gitlabFailedOldestRow!).getByText('Status').closest('div');
    expect(gitlabFailedOldestStatusRow).toBeInTheDocument();
    expect(within(gitlabFailedOldestStatusRow!).getByText('failed')).toBeInTheDocument();
    expect(within(gitlabFailedOldestRow!).queryByText('Status: failed')).not.toBeInTheDocument();
    expect(within(gitlabFailedOldestRow!).queryByText('succeeded')).not.toBeInTheDocument();
  });

  it('keeps sibling authenticated connection cards showing multiple terminal-state sync-history rows\' queued, started, and finished timestamps truthful and scoped within each card', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-failed-newest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-failed-newest',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:03:00Z',
            error: 'GitHub permissions denied',
          },
          {
            id: 'job-conn-2-succeeded-newest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-succeeded-newest',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T12:30:00Z',
            started_at: '2026-04-18T12:31:00Z',
            finished_at: '2026-04-18T12:33:00Z',
            error: null,
          },
          {
            id: 'job-conn-1-succeeded-older',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-succeeded-older',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:01:00Z',
            finished_at: '2026-04-18T11:02:00Z',
            error: null,
          },
          {
            id: 'job-conn-2-failed-older',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-failed-older',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T10:00:00Z',
            started_at: '2026-04-18T10:01:00Z',
            finished_at: '2026-04-18T10:04:00Z',
            error: 'GitLab import failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByText(/Queued at:/).map((node) => node.textContent)).toEqual([
      'Queued at: 2026-04-18T13:00:00Z',
      'Queued at: 2026-04-18T11:00:00Z',
    ]);
    expect(within(githubCard!).getAllByText(/Started at:/).map((node) => node.textContent)).toEqual([
      'Started at: 2026-04-18T13:01:00Z',
      'Started at: 2026-04-18T11:01:00Z',
    ]);
    expect(within(githubCard!).getAllByText(/Finished at:/).map((node) => node.textContent)).toEqual([
      'Finished at: 2026-04-18T13:03:00Z',
      'Finished at: 2026-04-18T11:02:00Z',
    ]);
    expect(within(githubCard!).queryByText('Queued at: 2026-04-18T12:30:00Z')).not.toBeInTheDocument();
    expect(within(githubCard!).queryByText('Started at: 2026-04-18T12:31:00Z')).not.toBeInTheDocument();
    expect(within(githubCard!).queryByText('Finished at: 2026-04-18T10:04:00Z')).not.toBeInTheDocument();

    expect(within(gitlabCard!).getAllByText(/Queued at:/).map((node) => node.textContent)).toEqual([
      'Queued at: 2026-04-18T12:30:00Z',
      'Queued at: 2026-04-18T10:00:00Z',
    ]);
    expect(within(gitlabCard!).getAllByText(/Started at:/).map((node) => node.textContent)).toEqual([
      'Started at: 2026-04-18T12:31:00Z',
      'Started at: 2026-04-18T10:01:00Z',
    ]);
    expect(within(gitlabCard!).getAllByText(/Finished at:/).map((node) => node.textContent)).toEqual([
      'Finished at: 2026-04-18T12:33:00Z',
      'Finished at: 2026-04-18T10:04:00Z',
    ]);
    expect(within(gitlabCard!).queryByText('Queued at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(gitlabCard!).queryByText('Started at: 2026-04-18T11:01:00Z')).not.toBeInTheDocument();
    expect(within(gitlabCard!).queryByText('Finished at: 2026-04-18T13:03:00Z')).not.toBeInTheDocument();
  });

  it('renders terminal-state sync-history rows with the shared status badge presentation across authenticated connection cards on the settings route', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-failed',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-failed',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:03:00Z',
            error: 'GitHub permissions denied',
          },
          {
            id: 'job-conn-2-succeeded',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-succeeded',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T12:30:00Z',
            started_at: '2026-04-18T12:31:00Z',
            finished_at: '2026-04-18T12:33:00Z',
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubLatestSyncSummary = within(githubCard!).getByText(/Latest sync:/).closest('div');
    expect(githubLatestSyncSummary).toBeInTheDocument();
    expect(within(githubLatestSyncSummary!).getByText('failed')).toBeInTheDocument();
    expect(within(githubLatestSyncSummary!).getByText('repo-conn-1-failed · 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(githubLatestSyncSummary!).queryByText('Latest sync: failed')).not.toBeInTheDocument();

    const githubTerminalSyncRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-conn-1-failed' }).closest('div');
    expect(githubTerminalSyncRow).toBeInTheDocument();
    const githubStatusRow = within(githubTerminalSyncRow!).getByText('Status').closest('div');
    expect(githubStatusRow).toBeInTheDocument();
    expect(within(githubStatusRow!).getByText('failed')).toBeInTheDocument();
    expect(within(githubTerminalSyncRow!).queryByText('Status: failed')).not.toBeInTheDocument();

    const gitlabLatestSyncSummary = within(gitlabCard!).getByText(/Latest sync:/).closest('div');
    expect(gitlabLatestSyncSummary).toBeInTheDocument();
    expect(within(gitlabLatestSyncSummary!).getByText('succeeded')).toBeInTheDocument();
    expect(within(gitlabLatestSyncSummary!).getByText('repo-conn-2-succeeded · 2026-04-18T12:30:00Z')).toBeInTheDocument();
    expect(within(gitlabLatestSyncSummary!).queryByText('Latest sync: succeeded')).not.toBeInTheDocument();

    const gitlabTerminalSyncRow = within(gitlabCard!).getByRole('link', { name: 'Open repository detail for repo-conn-2-succeeded' }).closest('div');
    expect(gitlabTerminalSyncRow).toBeInTheDocument();
    const gitlabStatusRow = within(gitlabTerminalSyncRow!).getByText('Status').closest('div');
    expect(gitlabStatusRow).toBeInTheDocument();
    expect(within(gitlabStatusRow!).getByText('succeeded')).toBeInTheDocument();
    expect(within(gitlabTerminalSyncRow!).queryByText('Status: succeeded')).not.toBeInTheDocument();
  });

  it('keeps terminal-state sync-history timestamp details scoped and truthful across authenticated connection cards on the settings route', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-failed',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-failed',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:03:00Z',
            error: 'GitHub permissions denied',
          },
          {
            id: 'job-conn-2-succeeded',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-succeeded',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T12:30:00Z',
            started_at: '2026-04-18T12:31:00Z',
            finished_at: '2026-04-18T12:33:00Z',
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubTerminalSyncRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-conn-1-failed' }).closest('div');
    expect(githubTerminalSyncRow).toBeInTheDocument();
    expect(within(githubTerminalSyncRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(githubTerminalSyncRow!).getByText('Started at: 2026-04-18T13:01:00Z')).toBeInTheDocument();
    expect(within(githubTerminalSyncRow!).getByText('Finished at: 2026-04-18T13:03:00Z')).toBeInTheDocument();
    expect(within(githubTerminalSyncRow!).queryByText('Queued at: 2026-04-18T12:30:00Z')).not.toBeInTheDocument();
    expect(within(githubTerminalSyncRow!).queryByText('Started at: 2026-04-18T12:31:00Z')).not.toBeInTheDocument();
    expect(within(githubTerminalSyncRow!).queryByText('Finished at: 2026-04-18T12:33:00Z')).not.toBeInTheDocument();

    const gitlabTerminalSyncRow = within(gitlabCard!).getByRole('link', { name: 'Open repository detail for repo-conn-2-succeeded' }).closest('div');
    expect(gitlabTerminalSyncRow).toBeInTheDocument();
    expect(within(gitlabTerminalSyncRow!).getByText('Queued at: 2026-04-18T12:30:00Z')).toBeInTheDocument();
    expect(within(gitlabTerminalSyncRow!).getByText('Started at: 2026-04-18T12:31:00Z')).toBeInTheDocument();
    expect(within(gitlabTerminalSyncRow!).getByText('Finished at: 2026-04-18T12:33:00Z')).toBeInTheDocument();
    expect(within(gitlabTerminalSyncRow!).queryByText('Queued at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(gitlabTerminalSyncRow!).queryByText('Started at: 2026-04-18T13:01:00Z')).not.toBeInTheDocument();
    expect(within(gitlabTerminalSyncRow!).queryByText('Finished at: 2026-04-18T13:03:00Z')).not.toBeInTheDocument();
  });

  it('keeps multiple succeeded and failed terminal-state sync-history rows on the same authenticated connection card using the shared status badge presentation truthfully and newest-first by queued_at', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-succeeded-newest',
            organization_id: 'org-1',
            repository_id: 'repo-succeeded-newest',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: null,
          },
          {
            id: 'job-failed-middle',
            organization_id: 'org-1',
            repository_id: 'repo-failed-middle',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: 'Mirror permissions denied',
          },
          {
            id: 'job-succeeded-oldest',
            organization_id: 'org-1',
            repository_id: 'repo-succeeded-oldest',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:03:00Z',
            finished_at: '2026-04-18T11:07:00Z',
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-succeeded-newest',
      '#/repos/repo-failed-middle',
      '#/repos/repo-succeeded-oldest',
    ]);

    const newestSucceededRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-succeeded-newest' }).closest('div');
    expect(newestSucceededRow).toBeInTheDocument();
    const newestSucceededStatusRow = within(newestSucceededRow!).getByText('Status').closest('div');
    expect(newestSucceededStatusRow).toBeInTheDocument();
    expect(within(newestSucceededStatusRow!).getByText('succeeded')).toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Status: succeeded')).not.toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('failed')).not.toBeInTheDocument();

    const middleFailedRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-failed-middle' }).closest('div');
    expect(middleFailedRow).toBeInTheDocument();
    const middleFailedStatusRow = within(middleFailedRow!).getByText('Status').closest('div');
    expect(middleFailedStatusRow).toBeInTheDocument();
    expect(within(middleFailedStatusRow!).getByText('failed')).toBeInTheDocument();
    expect(within(middleFailedRow!).queryByText('Status: failed')).not.toBeInTheDocument();
    expect(within(middleFailedRow!).queryByText('succeeded')).not.toBeInTheDocument();

    const oldestSucceededRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-succeeded-oldest' }).closest('div');
    expect(oldestSucceededRow).toBeInTheDocument();
    const oldestSucceededStatusRow = within(oldestSucceededRow!).getByText('Status').closest('div');
    expect(oldestSucceededStatusRow).toBeInTheDocument();
    expect(within(oldestSucceededStatusRow!).getByText('succeeded')).toBeInTheDocument();
    expect(within(oldestSucceededRow!).queryByText('Status: succeeded')).not.toBeInTheDocument();
    expect(within(oldestSucceededRow!).queryByText('failed')).not.toBeInTheDocument();
  });

  it('keeps multiple succeeded and failed terminal-state sync-history timestamp details truthful and newest-first on the same authenticated connection card', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-succeeded-newest',
            organization_id: 'org-1',
            repository_id: 'repo-succeeded-newest',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: null,
          },
          {
            id: 'job-failed-middle',
            organization_id: 'org-1',
            repository_id: 'repo-failed-middle',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: 'Mirror permissions denied',
          },
          {
            id: 'job-succeeded-oldest',
            organization_id: 'org-1',
            repository_id: 'repo-succeeded-oldest',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:03:00Z',
            finished_at: '2026-04-18T11:07:00Z',
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-succeeded-newest',
      '#/repos/repo-failed-middle',
      '#/repos/repo-succeeded-oldest',
    ]);
    expect(within(githubCard!).getAllByText(/Queued at:/).map((node) => node.textContent)).toEqual([
      'Queued at: 2026-04-18T13:00:00Z',
      'Queued at: 2026-04-18T12:00:00Z',
      'Queued at: 2026-04-18T11:00:00Z',
    ]);

    const latestSyncSummary = within(githubCard!).getByText(/Latest sync:/).closest('div');
    expect(latestSyncSummary).toBeInTheDocument();
    expect(within(latestSyncSummary!).getByText('succeeded')).toBeInTheDocument();
    expect(within(latestSyncSummary!).getByText('repo-succeeded-newest · 2026-04-18T13:00:00Z')).toBeInTheDocument();

    const newestSucceededRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-succeeded-newest' }).closest('div');
    expect(newestSucceededRow).toBeInTheDocument();
    expect(within(newestSucceededRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(newestSucceededRow!).getByText('Started at: 2026-04-18T13:01:00Z')).toBeInTheDocument();
    expect(within(newestSucceededRow!).getByText('Finished at: 2026-04-18T13:05:00Z')).toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Started at: 2026-04-18T12:02:00Z')).not.toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Finished at: 2026-04-18T12:06:00Z')).not.toBeInTheDocument();

    const middleFailedRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-failed-middle' }).closest('div');
    expect(middleFailedRow).toBeInTheDocument();
    expect(within(middleFailedRow!).getByText('Queued at: 2026-04-18T12:00:00Z')).toBeInTheDocument();
    expect(within(middleFailedRow!).getByText('Started at: 2026-04-18T12:02:00Z')).toBeInTheDocument();
    expect(within(middleFailedRow!).getByText('Finished at: 2026-04-18T12:06:00Z')).toBeInTheDocument();
    expect(within(middleFailedRow!).queryByText('Queued at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(middleFailedRow!).queryByText('Started at: 2026-04-18T13:01:00Z')).not.toBeInTheDocument();
    expect(within(middleFailedRow!).queryByText('Finished at: 2026-04-18T13:05:00Z')).not.toBeInTheDocument();
    expect(within(middleFailedRow!).queryByText('Queued at: 2026-04-18T11:00:00Z')).not.toBeInTheDocument();
    expect(within(middleFailedRow!).queryByText('Started at: 2026-04-18T11:03:00Z')).not.toBeInTheDocument();
    expect(within(middleFailedRow!).queryByText('Finished at: 2026-04-18T11:07:00Z')).not.toBeInTheDocument();

    const oldestSucceededRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-succeeded-oldest' }).closest('div');
    expect(oldestSucceededRow).toBeInTheDocument();
    expect(within(oldestSucceededRow!).getByText('Queued at: 2026-04-18T11:00:00Z')).toBeInTheDocument();
    expect(within(oldestSucceededRow!).getByText('Started at: 2026-04-18T11:03:00Z')).toBeInTheDocument();
    expect(within(oldestSucceededRow!).getByText('Finished at: 2026-04-18T11:07:00Z')).toBeInTheDocument();
    expect(within(oldestSucceededRow!).queryByText('Queued at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(oldestSucceededRow!).queryByText('Started at: 2026-04-18T13:01:00Z')).not.toBeInTheDocument();
    expect(within(oldestSucceededRow!).queryByText('Finished at: 2026-04-18T13:05:00Z')).not.toBeInTheDocument();
    expect(within(oldestSucceededRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();
    expect(within(oldestSucceededRow!).queryByText('Started at: 2026-04-18T12:02:00Z')).not.toBeInTheDocument();
    expect(within(oldestSucceededRow!).queryByText('Finished at: 2026-04-18T12:06:00Z')).not.toBeInTheDocument();
  });

  it('keeps multiple succeeded and failed terminal-state sync-history error details truthful and newest-first on the same authenticated connection card', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-failed-newest',
            organization_id: 'org-1',
            repository_id: 'repo-failed-newest',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'Newest mirror fetch failed',
          },
          {
            id: 'job-succeeded-middle',
            organization_id: 'org-1',
            repository_id: 'repo-succeeded-middle',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: null,
          },
          {
            id: 'job-failed-oldest',
            organization_id: 'org-1',
            repository_id: 'repo-failed-oldest',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:03:00Z',
            finished_at: '2026-04-18T11:07:00Z',
            error: 'Oldest mirror fetch failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-failed-newest',
      '#/repos/repo-succeeded-middle',
      '#/repos/repo-failed-oldest',
    ]);

    const newestFailedRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-failed-newest' }).closest('div');
    expect(newestFailedRow).toBeInTheDocument();
    expect(within(newestFailedRow!).getByText('Error: Newest mirror fetch failed')).toBeInTheDocument();
    expect(within(newestFailedRow!).queryByText('Error: Oldest mirror fetch failed')).not.toBeInTheDocument();
    expect(within(newestFailedRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();

    const middleSucceededRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-succeeded-middle' }).closest('div');
    expect(middleSucceededRow).toBeInTheDocument();
    expect(within(middleSucceededRow!).queryByText(/Error:/)).not.toBeInTheDocument();
    expect(within(middleSucceededRow!).queryByText('Error: Newest mirror fetch failed')).not.toBeInTheDocument();
    expect(within(middleSucceededRow!).queryByText('Error: Oldest mirror fetch failed')).not.toBeInTheDocument();
    expect(within(middleSucceededRow!).queryByText('Queued at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(middleSucceededRow!).queryByText('Queued at: 2026-04-18T11:00:00Z')).not.toBeInTheDocument();

    const oldestFailedRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-failed-oldest' }).closest('div');
    expect(oldestFailedRow).toBeInTheDocument();
    expect(within(oldestFailedRow!).getByText('Error: Oldest mirror fetch failed')).toBeInTheDocument();
    expect(within(oldestFailedRow!).queryByText('Error: Newest mirror fetch failed')).not.toBeInTheDocument();
    expect(within(oldestFailedRow!).queryByText('Queued at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
  });

  it('keeps multiple succeeded and failed terminal-state sync-history error details scoped and truthful across authenticated connection cards on the settings route', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-failed-newest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-failed-newest',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'GitHub newest mirror fetch failed',
          },
          {
            id: 'job-conn-2-succeeded-newest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-succeeded-newest',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T12:30:00Z',
            started_at: '2026-04-18T12:31:00Z',
            finished_at: '2026-04-18T12:35:00Z',
            error: null,
          },
          {
            id: 'job-conn-1-succeeded-older',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-succeeded-older',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:01:00Z',
            finished_at: '2026-04-18T11:04:00Z',
            error: null,
          },
          {
            id: 'job-conn-2-failed-older',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-failed-older',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T10:00:00Z',
            started_at: '2026-04-18T10:01:00Z',
            finished_at: '2026-04-18T10:03:00Z',
            error: 'GitLab oldest import failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-conn-1-failed-newest',
      '#/repos/repo-conn-1-succeeded-older',
    ]);
    const githubFailedRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-conn-1-failed-newest' }).closest('div');
    expect(githubFailedRow).toBeInTheDocument();
    expect(within(githubFailedRow!).getByText('Error: GitHub newest mirror fetch failed')).toBeInTheDocument();
    expect(within(githubFailedRow!).queryByText('Error: GitLab oldest import failed')).not.toBeInTheDocument();

    const githubSucceededRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-conn-1-succeeded-older' }).closest('div');
    expect(githubSucceededRow).toBeInTheDocument();
    expect(within(githubSucceededRow!).queryByText(/Error:/)).not.toBeInTheDocument();
    expect(within(githubSucceededRow!).queryByText('Error: GitHub newest mirror fetch failed')).not.toBeInTheDocument();
    expect(within(githubSucceededRow!).queryByText('Error: GitLab oldest import failed')).not.toBeInTheDocument();

    expect(within(gitlabCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-conn-2-succeeded-newest',
      '#/repos/repo-conn-2-failed-older',
    ]);
    const gitlabSucceededRow = within(gitlabCard!).getByRole('link', { name: 'Open repository detail for repo-conn-2-succeeded-newest' }).closest('div');
    expect(gitlabSucceededRow).toBeInTheDocument();
    expect(within(gitlabSucceededRow!).queryByText(/Error:/)).not.toBeInTheDocument();
    expect(within(gitlabSucceededRow!).queryByText('Error: GitHub newest mirror fetch failed')).not.toBeInTheDocument();
    expect(within(gitlabSucceededRow!).queryByText('Error: GitLab oldest import failed')).not.toBeInTheDocument();

    const gitlabFailedRow = within(gitlabCard!).getByRole('link', { name: 'Open repository detail for repo-conn-2-failed-older' }).closest('div');
    expect(gitlabFailedRow).toBeInTheDocument();
    expect(within(gitlabFailedRow!).getByText('Error: GitLab oldest import failed')).toBeInTheDocument();
    expect(within(gitlabFailedRow!).queryByText('Error: GitHub newest mirror fetch failed')).not.toBeInTheDocument();
  });

  it('keeps identical terminal-state sync-history error strings isolated across sibling authenticated connection cards', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-failed-newest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-failed-newest',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-conn-1-succeeded-older',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-succeeded-older',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:01:00Z',
            finished_at: '2026-04-18T11:04:00Z',
            error: null,
          },
          {
            id: 'job-conn-2-failed-newest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-failed-newest',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T12:30:00Z',
            started_at: '2026-04-18T12:31:00Z',
            finished_at: '2026-04-18T12:35:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-conn-2-succeeded-older',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-succeeded-older',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T10:00:00Z',
            started_at: '2026-04-18T10:01:00Z',
            finished_at: '2026-04-18T10:03:00Z',
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubFailedRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-conn-1-failed-newest' }).closest('div');
    const githubSucceededRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-conn-1-succeeded-older' }).closest('div');
    const gitlabFailedRow = within(gitlabCard!).getByRole('link', { name: 'Open repository detail for repo-conn-2-failed-newest' }).closest('div');
    const gitlabSucceededRow = within(gitlabCard!).getByRole('link', { name: 'Open repository detail for repo-conn-2-succeeded-older' }).closest('div');
    expect(githubFailedRow).toBeInTheDocument();
    expect(githubSucceededRow).toBeInTheDocument();
    expect(gitlabFailedRow).toBeInTheDocument();
    expect(gitlabSucceededRow).toBeInTheDocument();

    expect(within(githubFailedRow!).getByLabelText('Error details for repo-conn-1-failed-newest (failed)')).toHaveTextContent('Error: Mirror fetch failed');
    expect(within(gitlabFailedRow!).getByLabelText('Error details for repo-conn-2-failed-newest (failed)')).toHaveTextContent('Error: Mirror fetch failed');
    expect(within(githubSucceededRow!).queryByLabelText('Error details for repo-conn-1-succeeded-older (succeeded)')).not.toBeInTheDocument();
    expect(within(gitlabSucceededRow!).queryByLabelText('Error details for repo-conn-2-succeeded-older (succeeded)')).not.toBeInTheDocument();

    expect(within(githubFailedRow!).queryByLabelText('Error details for repo-conn-2-failed-newest (failed)')).not.toBeInTheDocument();
    expect(within(gitlabFailedRow!).queryByLabelText('Error details for repo-conn-1-failed-newest (failed)')).not.toBeInTheDocument();
    expect(within(githubCard!).getAllByText('Error: Mirror fetch failed')).toHaveLength(1);
    expect(within(gitlabCard!).getAllByText('Error: Mirror fetch failed')).toHaveLength(1);
  });

  it('keeps identical terminal-state sync-history error strings isolated within multiple failed rows on the same authenticated connection card', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-failed-newest',
            organization_id: 'org-1',
            repository_id: 'repo-failed-newest',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-failed-middle',
            organization_id: 'org-1',
            repository_id: 'repo-failed-middle',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-succeeded-oldest',
            organization_id: 'org-1',
            repository_id: 'repo-succeeded-oldest',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:01:00Z',
            finished_at: '2026-04-18T11:04:00Z',
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-failed-newest',
      '#/repos/repo-failed-middle',
      '#/repos/repo-succeeded-oldest',
    ]);

    const newestFailedRow = within(githubCard!).getByLabelText('Repository sync history row for repo-failed-newest (failed)');
    const middleFailedRow = within(githubCard!).getByLabelText('Repository sync history row for repo-failed-middle (failed)');
    const succeededRow = within(githubCard!).getByLabelText('Repository sync history row for repo-succeeded-oldest (succeeded)');

    expect(within(newestFailedRow).getByLabelText('Error details for repo-failed-newest (failed)')).toHaveTextContent('Error: Mirror fetch failed');
    expect(within(middleFailedRow).getByLabelText('Error details for repo-failed-middle (failed)')).toHaveTextContent('Error: Mirror fetch failed');
    expect(within(succeededRow).queryByText('Error: Mirror fetch failed')).not.toBeInTheDocument();

    expect(within(newestFailedRow).queryByLabelText('Error details for repo-failed-middle (failed)')).not.toBeInTheDocument();
    expect(within(middleFailedRow).queryByLabelText('Error details for repo-failed-newest (failed)')).not.toBeInTheDocument();
    expect(within(newestFailedRow).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();
    expect(within(middleFailedRow).queryByText('Queued at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(githubCard!).getAllByText('Error: Mirror fetch failed')).toHaveLength(2);
  });

  it('keeps the latest-sync summary truthful when multiple failed rows on the same authenticated connection card share the same terminal error string', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-failed-older',
            organization_id: 'org-1',
            repository_id: 'repo-failed-older',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-failed-newest',
            organization_id: 'org-1',
            repository_id: 'repo-failed-newest',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'Mirror fetch failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    const latestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(latestSyncSummary).toHaveTextContent('Latest sync:');
    expect(latestSyncSummary).toHaveTextContent('failed');
    expect(latestSyncSummary).toHaveTextContent('repo-failed-newest · 2026-04-18T13:00:00Z');
    expect(latestSyncSummary).not.toHaveTextContent('repo-failed-older · 2026-04-18T12:00:00Z');
  });

  it('keeps each sibling authenticated connection card latest-sync summary truthful to its own newest failed row when terminal error strings match', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-failed-older',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-failed-older',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-conn-2-failed-newest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-failed-newest',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T13:30:00Z',
            started_at: '2026-04-18T13:31:00Z',
            finished_at: '2026-04-18T13:35:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-conn-1-failed-newest',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-failed-newest',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-conn-2-failed-older',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-failed-older',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:02:00Z',
            finished_at: '2026-04-18T11:06:00Z',
            error: 'Mirror fetch failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubLatestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(githubLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(githubLatestSyncSummary).toHaveTextContent('failed');
    expect(githubLatestSyncSummary).toHaveTextContent('repo-conn-1-failed-newest · 2026-04-18T13:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-conn-1-failed-older · 2026-04-18T12:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-conn-2-failed-newest · 2026-04-18T13:30:00Z');

    const gitlabLatestSyncSummary = within(gitlabCard!).getByLabelText('Latest sync summary for GitLab Mirror');
    expect(gitlabLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(gitlabLatestSyncSummary).toHaveTextContent('failed');
    expect(gitlabLatestSyncSummary).toHaveTextContent('repo-conn-2-failed-newest · 2026-04-18T13:30:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-conn-2-failed-older · 2026-04-18T11:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-conn-1-failed-newest · 2026-04-18T13:00:00Z');
  });

  it('keeps sibling authenticated connection cards latest-sync summaries truthful when opposite mixed terminal-state histories end in failed-vs-succeeded newest rows', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-succeeded-older',
            organization_id: 'org-1',
            repository_id: 'repo-shared-history',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:01:00Z',
            finished_at: '2026-04-18T12:04:00Z',
            error: null,
          },
          {
            id: 'job-conn-2-failed-older',
            organization_id: 'org-1',
            repository_id: 'repo-shared-history',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:02:00Z',
            finished_at: '2026-04-18T11:06:00Z',
            error: 'GitLab import failed',
          },
          {
            id: 'job-conn-1-failed-newest',
            organization_id: 'org-1',
            repository_id: 'repo-shared-history',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'GitHub permissions denied',
          },
          {
            id: 'job-conn-2-succeeded-newest',
            organization_id: 'org-1',
            repository_id: 'repo-shared-history',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T13:30:00Z',
            started_at: '2026-04-18T13:31:00Z',
            finished_at: '2026-04-18T13:35:00Z',
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubLatestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(githubLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(githubLatestSyncSummary).toHaveTextContent('failed');
    expect(githubLatestSyncSummary).toHaveTextContent('repo-shared-history · 2026-04-18T13:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('succeeded');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-shared-history · 2026-04-18T12:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-shared-history · 2026-04-18T13:30:00Z');

    const gitlabLatestSyncSummary = within(gitlabCard!).getByLabelText('Latest sync summary for GitLab Mirror');
    expect(gitlabLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(gitlabLatestSyncSummary).toHaveTextContent('succeeded');
    expect(gitlabLatestSyncSummary).toHaveTextContent('repo-shared-history · 2026-04-18T13:30:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('failed');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-shared-history · 2026-04-18T11:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-shared-history · 2026-04-18T13:00:00Z');
  });

  it('keeps sibling authenticated connection cards latest-sync summaries truthful when opposite mixed terminal-state histories share the same newest queued_at timestamp across cards', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-succeeded-older-same-timestamp',
            organization_id: 'org-1',
            repository_id: 'repo-shared-same-timestamp-history',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:01:00Z',
            finished_at: '2026-04-18T12:04:00Z',
            error: null,
          },
          {
            id: 'job-conn-2-failed-older-same-timestamp',
            organization_id: 'org-1',
            repository_id: 'repo-shared-same-timestamp-history',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:02:00Z',
            finished_at: '2026-04-18T11:06:00Z',
            error: 'GitLab import failed',
          },
          {
            id: 'job-conn-1-failed-newest-same-timestamp',
            organization_id: 'org-1',
            repository_id: 'repo-shared-same-timestamp-history',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'GitHub permissions denied',
          },
          {
            id: 'job-conn-2-succeeded-newest-same-timestamp',
            organization_id: 'org-1',
            repository_id: 'repo-shared-same-timestamp-history',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubLatestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(githubLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(githubLatestSyncSummary).toHaveTextContent('failed');
    expect(githubLatestSyncSummary).toHaveTextContent('repo-shared-same-timestamp-history · 2026-04-18T13:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('succeeded');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-shared-same-timestamp-history · 2026-04-18T12:00:00Z');

    const gitlabLatestSyncSummary = within(gitlabCard!).getByLabelText('Latest sync summary for GitLab Mirror');
    expect(gitlabLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(gitlabLatestSyncSummary).toHaveTextContent('succeeded');
    expect(gitlabLatestSyncSummary).toHaveTextContent('repo-shared-same-timestamp-history · 2026-04-18T13:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('failed');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-shared-same-timestamp-history · 2026-04-18T11:00:00Z');
  });

  it('keeps sibling authenticated connection cards latest-sync summaries truthful when opposite mixed terminal-state histories share the same newest queued_at timestamp across cards and the API returns those tied newest rows in reverse order', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-2-succeeded-newest-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-same-timestamp-history',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: null,
          },
          {
            id: 'job-conn-1-failed-newest-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-same-timestamp-history',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'GitHub permissions denied',
          },
          {
            id: 'job-conn-2-failed-older-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-same-timestamp-history',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:02:00Z',
            finished_at: '2026-04-18T11:06:00Z',
            error: 'GitLab import failed',
          },
          {
            id: 'job-conn-1-succeeded-older-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-same-timestamp-history',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:01:00Z',
            finished_at: '2026-04-18T12:04:00Z',
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubLatestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(githubLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(githubLatestSyncSummary).toHaveTextContent('failed');
    expect(githubLatestSyncSummary).toHaveTextContent('repo-shared-same-timestamp-history · 2026-04-18T13:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('succeeded');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-shared-same-timestamp-history · 2026-04-18T12:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('GitLab import failed');

    const gitlabLatestSyncSummary = within(gitlabCard!).getByLabelText('Latest sync summary for GitLab Mirror');
    expect(gitlabLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(gitlabLatestSyncSummary).toHaveTextContent('succeeded');
    expect(gitlabLatestSyncSummary).toHaveTextContent('repo-shared-same-timestamp-history · 2026-04-18T13:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('failed');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-shared-same-timestamp-history · 2026-04-18T11:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('GitHub permissions denied');
  });

  it('keeps sibling authenticated connection cards latest-sync summaries truthful when opposite mixed terminal-state histories share the same newest queued_at timestamp across cards, reuse the same repository id, and arrive in reverse API order', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-2-succeeded-newest-shared-repo-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-same-timestamp',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: null,
          },
          {
            id: 'job-conn-1-failed-newest-shared-repo-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-same-timestamp',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'GitHub permissions denied',
          },
          {
            id: 'job-conn-2-failed-older-shared-repo-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-older-shared-repo-same-timestamp',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:02:00Z',
            finished_at: '2026-04-18T11:06:00Z',
            error: 'GitLab import failed',
          },
          {
            id: 'job-conn-1-succeeded-older-shared-repo-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-github-older-shared-repo-same-timestamp',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:01:00Z',
            finished_at: '2026-04-18T12:04:00Z',
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    expect(await screen.findByText('GitHub Cloud')).toBeInTheDocument();
    expect(await screen.findByText('GitLab Mirror')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubLatestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(githubLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(githubLatestSyncSummary).toHaveTextContent('failed');
    expect(githubLatestSyncSummary).toHaveTextContent('repo-sibling-shared-newest-same-timestamp · 2026-04-18T13:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('succeeded');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-github-older-shared-repo-same-timestamp · 2026-04-18T12:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('GitLab import failed');

    const gitlabLatestSyncSummary = within(gitlabCard!).getByLabelText('Latest sync summary for GitLab Mirror');
    expect(gitlabLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(gitlabLatestSyncSummary).toHaveTextContent('succeeded');
    expect(gitlabLatestSyncSummary).toHaveTextContent('repo-sibling-shared-newest-same-timestamp · 2026-04-18T13:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('failed');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-gitlab-older-shared-repo-same-timestamp · 2026-04-18T11:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('GitHub permissions denied');
  });

  it('keeps sibling authenticated connection cards latest-sync summaries truthful when opposite mixed terminal-state histories share the same newest queued_at timestamp across cards, already arrive newest-first, and also reuse the same repository id', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-failed-newest-shared-repo-same-timestamp-stable-order',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-same-timestamp-stable-order',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'GitHub permissions denied',
          },
          {
            id: 'job-conn-2-succeeded-newest-shared-repo-same-timestamp-stable-order',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-same-timestamp-stable-order',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: null,
          },
          {
            id: 'job-conn-1-succeeded-older-shared-repo-same-timestamp-stable-order',
            organization_id: 'org-1',
            repository_id: 'repo-github-older-shared-repo-same-timestamp-stable-order',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:01:00Z',
            finished_at: '2026-04-18T12:04:00Z',
            error: null,
          },
          {
            id: 'job-conn-2-failed-older-shared-repo-same-timestamp-stable-order',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-older-shared-repo-same-timestamp-stable-order',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:02:00Z',
            finished_at: '2026-04-18T11:06:00Z',
            error: 'GitLab import failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    expect(await screen.findByText('GitHub Cloud')).toBeInTheDocument();
    expect(await screen.findByText('GitLab Mirror')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubLatestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(githubLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(githubLatestSyncSummary).toHaveTextContent('failed');
    expect(githubLatestSyncSummary).toHaveTextContent('repo-sibling-shared-newest-same-timestamp-stable-order · 2026-04-18T13:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('succeeded');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-github-older-shared-repo-same-timestamp-stable-order · 2026-04-18T12:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('GitLab import failed');

    const gitlabLatestSyncSummary = within(gitlabCard!).getByLabelText('Latest sync summary for GitLab Mirror');
    expect(gitlabLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(gitlabLatestSyncSummary).toHaveTextContent('succeeded');
    expect(gitlabLatestSyncSummary).toHaveTextContent('repo-sibling-shared-newest-same-timestamp-stable-order · 2026-04-18T13:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('failed');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-gitlab-older-shared-repo-same-timestamp-stable-order · 2026-04-18T11:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('GitHub permissions denied');
  });

  it('keeps sibling authenticated connection cards latest-sync summaries truthful when opposite mixed terminal-state histories share both newest queued_at and activity timestamps across cards, already arrive newest-first, and also reuse the same repository id', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-failed-newest-shared-repo-exact-timestamp-stable-order',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-exact-timestamp-stable-order',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: 'GitHub permissions denied',
          },
          {
            id: 'job-conn-1-succeeded-newest-shared-repo-exact-timestamp-stable-order',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-exact-timestamp-stable-order',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: null,
          },
          {
            id: 'job-conn-2-succeeded-newest-shared-repo-exact-timestamp-stable-order',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-exact-timestamp-stable-order',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: null,
          },
          {
            id: 'job-conn-2-failed-newest-shared-repo-exact-timestamp-stable-order',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-exact-timestamp-stable-order',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: 'GitLab import failed',
          },
          {
            id: 'job-conn-1-succeeded-older-shared-repo-exact-timestamp-stable-order',
            organization_id: 'org-1',
            repository_id: 'repo-github-older-shared-repo-exact-timestamp-stable-order',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: null,
          },
          {
            id: 'job-conn-2-failed-older-shared-repo-exact-timestamp-stable-order',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-older-shared-repo-exact-timestamp-stable-order',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:02:00Z',
            finished_at: '2026-04-18T11:06:00Z',
            error: 'GitLab import failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    expect(await screen.findByText('GitHub Cloud')).toBeInTheDocument();
    expect(await screen.findByText('GitLab Mirror')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubLatestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(githubLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(githubLatestSyncSummary).toHaveTextContent('failed');
    expect(githubLatestSyncSummary).toHaveTextContent('repo-sibling-shared-newest-exact-timestamp-stable-order · 2026-04-18T13:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('succeeded');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-github-older-shared-repo-exact-timestamp-stable-order · 2026-04-18T12:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('GitLab import failed');

    const gitlabLatestSyncSummary = within(gitlabCard!).getByLabelText('Latest sync summary for GitLab Mirror');
    expect(gitlabLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(gitlabLatestSyncSummary).toHaveTextContent('succeeded');
    expect(gitlabLatestSyncSummary).toHaveTextContent('repo-sibling-shared-newest-exact-timestamp-stable-order · 2026-04-18T13:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('failed');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-gitlab-older-shared-repo-exact-timestamp-stable-order · 2026-04-18T11:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('GitHub permissions denied');
  });

  it('keeps sibling authenticated connection cards latest-sync summaries truthful when opposite mixed terminal-state histories share both newest queued_at and activity timestamps across cards, reuse the same repository id, and arrive in reverse API order', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-succeeded-newest-shared-repo-exact-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-exact-timestamp-reverse-order',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: null,
          },
          {
            id: 'job-conn-1-failed-newest-shared-repo-exact-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-exact-timestamp-reverse-order',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: 'GitHub permissions denied',
          },
          {
            id: 'job-conn-2-failed-newest-shared-repo-exact-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-exact-timestamp-reverse-order',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: 'GitLab import failed',
          },
          {
            id: 'job-conn-2-succeeded-newest-shared-repo-exact-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-exact-timestamp-reverse-order',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: null,
          },
          {
            id: 'job-conn-1-succeeded-older-shared-repo-exact-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-github-older-shared-repo-exact-timestamp-reverse-order',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: null,
          },
          {
            id: 'job-conn-2-failed-older-shared-repo-exact-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-older-shared-repo-exact-timestamp-reverse-order',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:02:00Z',
            finished_at: '2026-04-18T11:06:00Z',
            error: 'GitLab import failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    expect(await screen.findByText('GitHub Cloud')).toBeInTheDocument();
    expect(await screen.findByText('GitLab Mirror')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubLatestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(githubLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(githubLatestSyncSummary).toHaveTextContent('succeeded');
    expect(githubLatestSyncSummary).toHaveTextContent('repo-sibling-shared-newest-exact-timestamp-reverse-order · 2026-04-18T13:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('failed');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-github-older-shared-repo-exact-timestamp-reverse-order · 2026-04-18T12:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('GitLab import failed');

    const gitlabLatestSyncSummary = within(gitlabCard!).getByLabelText('Latest sync summary for GitLab Mirror');
    expect(gitlabLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(gitlabLatestSyncSummary).toHaveTextContent('failed');
    expect(gitlabLatestSyncSummary).toHaveTextContent('repo-sibling-shared-newest-exact-timestamp-reverse-order · 2026-04-18T13:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('succeeded');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-gitlab-older-shared-repo-exact-timestamp-reverse-order · 2026-04-18T11:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('GitHub permissions denied');
  });

  it('keeps sibling authenticated connection cards terminal-state sync-history timestamp details truthful when opposite mixed terminal-state histories share both newest queued_at and activity timestamps across cards, reuse the same repository id, and arrive in reverse API order', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-succeeded-newest-shared-repo-exact-timestamp-reverse-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-exact-timestamp-reverse-order-details',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: null,
          },
          {
            id: 'job-conn-1-failed-newest-shared-repo-exact-timestamp-reverse-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-exact-timestamp-reverse-order-details',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: 'GitHub permissions denied',
          },
          {
            id: 'job-conn-2-failed-newest-shared-repo-exact-timestamp-reverse-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-exact-timestamp-reverse-order-details',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: 'GitLab import failed',
          },
          {
            id: 'job-conn-2-succeeded-newest-shared-repo-exact-timestamp-reverse-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-exact-timestamp-reverse-order-details',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: null,
          },
          {
            id: 'job-conn-1-succeeded-older-shared-repo-exact-timestamp-reverse-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-github-older-shared-repo-exact-timestamp-reverse-order-details',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: null,
          },
          {
            id: 'job-conn-2-failed-older-shared-repo-exact-timestamp-reverse-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-older-shared-repo-exact-timestamp-reverse-order-details',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:02:00Z',
            finished_at: '2026-04-18T11:06:00Z',
            error: 'GitLab import failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    expect(await screen.findByText('GitHub Cloud')).toBeInTheDocument();
    expect(await screen.findByText('GitLab Mirror')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-sibling-shared-newest-exact-timestamp-reverse-order-details',
      '#/repos/repo-sibling-shared-newest-exact-timestamp-reverse-order-details',
      '#/repos/repo-github-older-shared-repo-exact-timestamp-reverse-order-details',
    ]);
    expect(within(gitlabCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-sibling-shared-newest-exact-timestamp-reverse-order-details',
      '#/repos/repo-sibling-shared-newest-exact-timestamp-reverse-order-details',
      '#/repos/repo-gitlab-older-shared-repo-exact-timestamp-reverse-order-details',
    ]);

    const githubTerminalRows = within(githubCard!).getAllByRole('link', {
      name: 'Open repository detail for repo-sibling-shared-newest-exact-timestamp-reverse-order-details',
    });
    expect(githubTerminalRows).toHaveLength(2);

    const githubNewestSucceededRow = githubTerminalRows[0].closest('div');
    expect(githubNewestSucceededRow).toBeInTheDocument();
    expect(within(githubNewestSucceededRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(githubNewestSucceededRow!).getByText('Started at: 2026-04-18T13:02:00Z')).toBeInTheDocument();
    expect(within(githubNewestSucceededRow!).getByText('Finished at: 2026-04-18T13:06:00Z')).toBeInTheDocument();
    expect(within(githubNewestSucceededRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();
    expect(within(githubNewestSucceededRow!).queryByText('Started at: 2026-04-18T12:02:00Z')).not.toBeInTheDocument();
    expect(within(githubNewestSucceededRow!).queryByText('Finished at: 2026-04-18T12:06:00Z')).not.toBeInTheDocument();

    const githubNewestFailedRow = githubTerminalRows[1].closest('div');
    expect(githubNewestFailedRow).toBeInTheDocument();
    expect(within(githubNewestFailedRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(githubNewestFailedRow!).getByText('Started at: 2026-04-18T13:02:00Z')).toBeInTheDocument();
    expect(within(githubNewestFailedRow!).getByText('Finished at: 2026-04-18T13:06:00Z')).toBeInTheDocument();
    expect(within(githubNewestFailedRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();
    expect(within(githubNewestFailedRow!).queryByText('Started at: 2026-04-18T12:02:00Z')).not.toBeInTheDocument();
    expect(within(githubNewestFailedRow!).queryByText('Finished at: 2026-04-18T12:06:00Z')).not.toBeInTheDocument();

    const gitlabTerminalRows = within(gitlabCard!).getAllByRole('link', {
      name: 'Open repository detail for repo-sibling-shared-newest-exact-timestamp-reverse-order-details',
    });
    expect(gitlabTerminalRows).toHaveLength(2);

    const gitlabNewestSucceededRow = gitlabTerminalRows[0].closest('div');
    expect(gitlabNewestSucceededRow).toBeInTheDocument();
    expect(within(gitlabNewestSucceededRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(gitlabNewestSucceededRow!).getByText('Started at: 2026-04-18T13:02:00Z')).toBeInTheDocument();
    expect(within(gitlabNewestSucceededRow!).getByText('Finished at: 2026-04-18T13:06:00Z')).toBeInTheDocument();
    expect(within(gitlabNewestSucceededRow!).queryByText('Queued at: 2026-04-18T11:00:00Z')).not.toBeInTheDocument();
    expect(within(gitlabNewestSucceededRow!).queryByText('Started at: 2026-04-18T11:02:00Z')).not.toBeInTheDocument();
    expect(within(gitlabNewestSucceededRow!).queryByText('Finished at: 2026-04-18T11:06:00Z')).not.toBeInTheDocument();

    const gitlabNewestFailedRow = gitlabTerminalRows[1].closest('div');
    expect(gitlabNewestFailedRow).toBeInTheDocument();
    expect(within(gitlabNewestFailedRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(gitlabNewestFailedRow!).getByText('Started at: 2026-04-18T13:02:00Z')).toBeInTheDocument();
    expect(within(gitlabNewestFailedRow!).getByText('Finished at: 2026-04-18T13:06:00Z')).toBeInTheDocument();
    expect(within(gitlabNewestFailedRow!).queryByText('Queued at: 2026-04-18T11:00:00Z')).not.toBeInTheDocument();
    expect(within(gitlabNewestFailedRow!).queryByText('Started at: 2026-04-18T11:02:00Z')).not.toBeInTheDocument();
    expect(within(gitlabNewestFailedRow!).queryByText('Finished at: 2026-04-18T11:06:00Z')).not.toBeInTheDocument();
  });

  it('keeps sibling authenticated connection cards terminal-state sync-history timestamp details truthful when opposite mixed terminal-state histories share both newest queued_at and activity timestamps across cards, already arrive newest-first, and also reuse the same repository id', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-failed-newest-shared-repo-same-timestamp-stable-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-same-timestamp-stable-order-details',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'GitHub permissions denied',
          },
          {
            id: 'job-conn-2-succeeded-newest-shared-repo-same-timestamp-stable-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-same-timestamp-stable-order-details',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: null,
          },
          {
            id: 'job-conn-1-succeeded-older-shared-repo-same-timestamp-stable-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-github-older-shared-repo-same-timestamp-stable-order-details',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:01:00Z',
            finished_at: '2026-04-18T12:04:00Z',
            error: null,
          },
          {
            id: 'job-conn-2-failed-older-shared-repo-same-timestamp-stable-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-older-shared-repo-same-timestamp-stable-order-details',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:02:00Z',
            finished_at: '2026-04-18T11:06:00Z',
            error: 'GitLab import failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    expect(await screen.findByText('GitHub Cloud')).toBeInTheDocument();
    expect(await screen.findByText('GitLab Mirror')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-sibling-shared-newest-same-timestamp-stable-order-details',
      '#/repos/repo-github-older-shared-repo-same-timestamp-stable-order-details',
    ]);
    expect(within(gitlabCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-sibling-shared-newest-same-timestamp-stable-order-details',
      '#/repos/repo-gitlab-older-shared-repo-same-timestamp-stable-order-details',
    ]);

    const githubNewestTerminalRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-sibling-shared-newest-same-timestamp-stable-order-details' }).closest('div');
    expect(githubNewestTerminalRow).toBeInTheDocument();
    expect(within(githubNewestTerminalRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(githubNewestTerminalRow!).getByText('Started at: 2026-04-18T13:01:00Z')).toBeInTheDocument();
    expect(within(githubNewestTerminalRow!).getByText('Finished at: 2026-04-18T13:05:00Z')).toBeInTheDocument();
    expect(within(githubNewestTerminalRow!).queryByText('Started at: 2026-04-18T12:01:00Z')).not.toBeInTheDocument();
    expect(within(githubNewestTerminalRow!).queryByText('Finished at: 2026-04-18T12:04:00Z')).not.toBeInTheDocument();
    expect(within(githubNewestTerminalRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();

    const gitlabNewestTerminalRow = within(gitlabCard!).getByRole('link', { name: 'Open repository detail for repo-sibling-shared-newest-same-timestamp-stable-order-details' }).closest('div');
    expect(gitlabNewestTerminalRow).toBeInTheDocument();
    expect(within(gitlabNewestTerminalRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(gitlabNewestTerminalRow!).getByText('Started at: 2026-04-18T13:01:00Z')).toBeInTheDocument();
    expect(within(gitlabNewestTerminalRow!).getByText('Finished at: 2026-04-18T13:05:00Z')).toBeInTheDocument();
    expect(within(gitlabNewestTerminalRow!).queryByText('Started at: 2026-04-18T11:02:00Z')).not.toBeInTheDocument();
    expect(within(gitlabNewestTerminalRow!).queryByText('Finished at: 2026-04-18T11:06:00Z')).not.toBeInTheDocument();
    expect(within(gitlabNewestTerminalRow!).queryByText('Queued at: 2026-04-18T11:00:00Z')).not.toBeInTheDocument();
  });

  it('keeps sibling authenticated connection cards terminal-state sync-history error details truthful when opposite mixed terminal-state histories share both newest queued_at and activity timestamps across cards, reuse the same repository id, and arrive in reverse API order', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-succeeded-newest-shared-repo-exact-timestamp-reverse-order-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-exact-timestamp-reverse-order-error-details',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: null,
          },
          {
            id: 'job-conn-1-failed-newest-shared-repo-exact-timestamp-reverse-order-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-exact-timestamp-reverse-order-error-details',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: 'GitHub permissions denied',
          },
          {
            id: 'job-conn-2-failed-newest-shared-repo-exact-timestamp-reverse-order-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-exact-timestamp-reverse-order-error-details',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: 'GitLab import failed',
          },
          {
            id: 'job-conn-2-succeeded-newest-shared-repo-exact-timestamp-reverse-order-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-exact-timestamp-reverse-order-error-details',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: null,
          },
          {
            id: 'job-conn-1-succeeded-older-shared-repo-exact-timestamp-reverse-order-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-github-older-shared-repo-exact-timestamp-reverse-order-error-details',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: null,
          },
          {
            id: 'job-conn-2-failed-older-shared-repo-exact-timestamp-reverse-order-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-older-shared-repo-exact-timestamp-reverse-order-error-details',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:02:00Z',
            finished_at: '2026-04-18T11:06:00Z',
            error: 'GitLab import failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    expect(await screen.findByText('GitHub Cloud')).toBeInTheDocument();
    expect(await screen.findByText('GitLab Mirror')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-sibling-shared-newest-exact-timestamp-reverse-order-error-details',
      '#/repos/repo-sibling-shared-newest-exact-timestamp-reverse-order-error-details',
      '#/repos/repo-github-older-shared-repo-exact-timestamp-reverse-order-error-details',
    ]);
    expect(within(gitlabCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-sibling-shared-newest-exact-timestamp-reverse-order-error-details',
      '#/repos/repo-sibling-shared-newest-exact-timestamp-reverse-order-error-details',
      '#/repos/repo-gitlab-older-shared-repo-exact-timestamp-reverse-order-error-details',
    ]);

    const githubNewestSucceededRow = within(githubCard!).getByLabelText(
      'Repository sync history row for repo-sibling-shared-newest-exact-timestamp-reverse-order-error-details (succeeded)',
    );
    expect(githubNewestSucceededRow).toBeInTheDocument();
    expect(within(githubNewestSucceededRow).queryByText(/Error:/)).not.toBeInTheDocument();
    expect(within(githubNewestSucceededRow).queryByText('Error: GitHub permissions denied')).not.toBeInTheDocument();
    expect(within(githubNewestSucceededRow).queryByText('Error: GitLab import failed')).not.toBeInTheDocument();

    const githubNewestFailedRow = within(githubCard!).getByLabelText(
      'Repository sync history row for repo-sibling-shared-newest-exact-timestamp-reverse-order-error-details (failed)',
    );
    expect(githubNewestFailedRow).toBeInTheDocument();
    expect(within(githubNewestFailedRow).getByLabelText(
      'Error details for repo-sibling-shared-newest-exact-timestamp-reverse-order-error-details (failed)',
    )).toHaveTextContent('Error: GitHub permissions denied');
    expect(within(githubNewestFailedRow).queryByText('Error: GitLab import failed')).not.toBeInTheDocument();

    const githubOlderTerminalRow = within(githubCard!).getByLabelText(
      'Repository sync history row for repo-github-older-shared-repo-exact-timestamp-reverse-order-error-details (succeeded)',
    );
    expect(githubOlderTerminalRow).toBeInTheDocument();
    expect(within(githubOlderTerminalRow).queryByText(/Error:/)).not.toBeInTheDocument();
    expect(within(githubOlderTerminalRow).queryByText('Error: GitHub permissions denied')).not.toBeInTheDocument();

    const gitlabNewestSucceededRow = within(gitlabCard!).getByLabelText(
      'Repository sync history row for repo-sibling-shared-newest-exact-timestamp-reverse-order-error-details (succeeded)',
    );
    expect(gitlabNewestSucceededRow).toBeInTheDocument();
    expect(within(gitlabNewestSucceededRow).queryByText(/Error:/)).not.toBeInTheDocument();
    expect(within(gitlabNewestSucceededRow).queryByText('Error: GitHub permissions denied')).not.toBeInTheDocument();
    expect(within(gitlabNewestSucceededRow).queryByText('Error: GitLab import failed')).not.toBeInTheDocument();

    const gitlabNewestFailedRow = within(gitlabCard!).getByLabelText(
      'Repository sync history row for repo-sibling-shared-newest-exact-timestamp-reverse-order-error-details (failed)',
    );
    expect(gitlabNewestFailedRow).toBeInTheDocument();
    expect(within(gitlabNewestFailedRow).getByLabelText(
      'Error details for repo-sibling-shared-newest-exact-timestamp-reverse-order-error-details (failed)',
    )).toHaveTextContent('Error: GitLab import failed');
    expect(within(gitlabNewestFailedRow).queryByText('Error: GitHub permissions denied')).not.toBeInTheDocument();

    const gitlabOlderTerminalRow = within(gitlabCard!).getByLabelText(
      'Repository sync history row for repo-gitlab-older-shared-repo-exact-timestamp-reverse-order-error-details (failed)',
    );
    expect(gitlabOlderTerminalRow).toBeInTheDocument();
    expect(within(gitlabOlderTerminalRow).getByLabelText(
      'Error details for repo-gitlab-older-shared-repo-exact-timestamp-reverse-order-error-details (failed)',
    )).toHaveTextContent('Error: GitLab import failed');
    expect(within(gitlabOlderTerminalRow).queryByText('Error: GitHub permissions denied')).not.toBeInTheDocument();
  });

  it('keeps sibling authenticated connection cards terminal-state sync-history error details truthful when opposite mixed terminal-state histories share both newest queued_at and activity timestamps across cards, already arrive newest-first, and also reuse the same repository id', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-failed-newest-shared-repo-same-timestamp-stable-order-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-same-timestamp-stable-order-error-details',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'GitHub permissions denied',
          },
          {
            id: 'job-conn-2-succeeded-newest-shared-repo-same-timestamp-stable-order-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-same-timestamp-stable-order-error-details',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: null,
          },
          {
            id: 'job-conn-1-succeeded-older-shared-repo-same-timestamp-stable-order-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-github-older-shared-repo-same-timestamp-stable-order-error-details',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:01:00Z',
            finished_at: '2026-04-18T12:04:00Z',
            error: null,
          },
          {
            id: 'job-conn-2-failed-older-shared-repo-same-timestamp-stable-order-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-older-shared-repo-same-timestamp-stable-order-error-details',
            connection_id: 'conn-2',
            status: 'failed',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:02:00Z',
            finished_at: '2026-04-18T11:06:00Z',
            error: 'GitLab import failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    expect(await screen.findByText('GitHub Cloud')).toBeInTheDocument();
    expect(await screen.findByText('GitLab Mirror')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-sibling-shared-newest-same-timestamp-stable-order-error-details',
      '#/repos/repo-github-older-shared-repo-same-timestamp-stable-order-error-details',
    ]);
    expect(within(gitlabCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-sibling-shared-newest-same-timestamp-stable-order-error-details',
      '#/repos/repo-gitlab-older-shared-repo-same-timestamp-stable-order-error-details',
    ]);

    const githubNewestTerminalRow = within(githubCard!).getByRole('link', {
      name: 'Open repository detail for repo-sibling-shared-newest-same-timestamp-stable-order-error-details',
    }).closest('div');
    expect(githubNewestTerminalRow).toBeInTheDocument();
    expect(within(githubNewestTerminalRow!).getByText('Error: GitHub permissions denied')).toBeInTheDocument();
    expect(within(githubNewestTerminalRow!).queryByText('Error: GitLab import failed')).not.toBeInTheDocument();

    const githubOlderTerminalRow = within(githubCard!).getByRole('link', {
      name: 'Open repository detail for repo-github-older-shared-repo-same-timestamp-stable-order-error-details',
    }).closest('div');
    expect(githubOlderTerminalRow).toBeInTheDocument();
    expect(within(githubOlderTerminalRow!).queryByText(/Error:/)).not.toBeInTheDocument();
    expect(within(githubOlderTerminalRow!).queryByText('Error: GitHub permissions denied')).not.toBeInTheDocument();

    const gitlabNewestTerminalRow = within(gitlabCard!).getByRole('link', {
      name: 'Open repository detail for repo-sibling-shared-newest-same-timestamp-stable-order-error-details',
    }).closest('div');
    expect(gitlabNewestTerminalRow).toBeInTheDocument();
    expect(within(gitlabNewestTerminalRow!).queryByText(/Error:/)).not.toBeInTheDocument();
    expect(within(gitlabNewestTerminalRow!).queryByText('Error: GitHub permissions denied')).not.toBeInTheDocument();
    expect(within(gitlabNewestTerminalRow!).queryByText('Error: GitLab import failed')).not.toBeInTheDocument();

    const gitlabOlderTerminalRow = within(gitlabCard!).getByRole('link', {
      name: 'Open repository detail for repo-gitlab-older-shared-repo-same-timestamp-stable-order-error-details',
    }).closest('div');
    expect(gitlabOlderTerminalRow).toBeInTheDocument();
    expect(within(gitlabOlderTerminalRow!).getByText('Error: GitLab import failed')).toBeInTheDocument();
    expect(within(gitlabOlderTerminalRow!).queryByText('Error: GitHub permissions denied')).not.toBeInTheDocument();
  });

  it('keeps the latest-sync summary truthful when the same authenticated connection card has opposite mixed terminal-state rows sharing the same newest queued_at timestamp and the API returns them in reverse order', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-failed-newest-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-failed-newest-same-timestamp',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-succeeded-newest-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-succeeded-newest-same-timestamp',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: null,
          },
          {
            id: 'job-failed-older-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-failed-older',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: 'Mirror fetch failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    const latestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(latestSyncSummary).toHaveTextContent('Latest sync:');
    expect(latestSyncSummary).toHaveTextContent('succeeded');
    expect(latestSyncSummary).toHaveTextContent('repo-succeeded-newest-same-timestamp · 2026-04-18T13:00:00Z');
    expect(latestSyncSummary).not.toHaveTextContent('failed');
    expect(latestSyncSummary).not.toHaveTextContent('repo-failed-newest-same-timestamp · 2026-04-18T13:00:00Z');
    expect(latestSyncSummary).not.toHaveTextContent('repo-failed-older · 2026-04-18T12:00:00Z');
  });

  it('keeps the latest-sync summary truthful when the same authenticated connection card has opposite mixed terminal-state rows sharing the same newest queued_at timestamp, reusing the same repository id, and the API returns them in reverse order', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-failed-newest-shared-repo-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-same-timestamp',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-succeeded-newest-shared-repo-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-same-timestamp',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: null,
          },
          {
            id: 'job-failed-older-shared-repo-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-older',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: 'Mirror fetch failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    const latestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(latestSyncSummary).toHaveTextContent('Latest sync:');
    expect(latestSyncSummary).toHaveTextContent('succeeded');
    expect(latestSyncSummary).toHaveTextContent('repo-shared-newest-same-timestamp · 2026-04-18T13:00:00Z');
    expect(latestSyncSummary).not.toHaveTextContent('failed');
    expect(latestSyncSummary).not.toHaveTextContent('repo-shared-older · 2026-04-18T12:00:00Z');
  });

  it('keeps the latest-sync summary truthful when the same authenticated connection card has opposite mixed terminal-state rows sharing the same newest queued_at timestamp, already arrive newest-first, and also reuse the same repository id', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-succeeded-newest-shared-repo-same-timestamp-stable-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-same-timestamp-stable-order',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: null,
          },
          {
            id: 'job-failed-newest-shared-repo-same-timestamp-stable-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-same-timestamp-stable-order',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-failed-older-shared-repo-same-timestamp-stable-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-older-stable-order',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: 'Mirror fetch failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    const latestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(latestSyncSummary).toHaveTextContent('Latest sync:');
    expect(latestSyncSummary).toHaveTextContent('succeeded');
    expect(latestSyncSummary).toHaveTextContent('repo-shared-newest-same-timestamp-stable-order · 2026-04-18T13:00:00Z');
    expect(latestSyncSummary).not.toHaveTextContent('failed');
    expect(latestSyncSummary).not.toHaveTextContent('repo-shared-older-stable-order · 2026-04-18T12:00:00Z');
  });

  it('keeps the latest-sync summary truthful when the same authenticated connection card has opposite mixed terminal-state rows sharing both newest queued_at and activity timestamps, already arrive newest-first, and also reuse the same repository id', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-failed-newest-shared-repo-exact-timestamp-stable-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-exact-timestamp-stable-order',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-succeeded-newest-shared-repo-exact-timestamp-stable-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-exact-timestamp-stable-order',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: null,
          },
          {
            id: 'job-succeeded-older-shared-repo-exact-timestamp-stable-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-older-exact-timestamp-stable-order',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    const latestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(latestSyncSummary).toHaveTextContent('Latest sync:');
    expect(latestSyncSummary).toHaveTextContent('failed');
    expect(latestSyncSummary).toHaveTextContent('repo-shared-newest-exact-timestamp-stable-order · 2026-04-18T13:00:00Z');
    expect(latestSyncSummary).not.toHaveTextContent('succeeded');
    expect(latestSyncSummary).not.toHaveTextContent('repo-shared-older-exact-timestamp-stable-order · 2026-04-18T12:00:00Z');
  });

  it('keeps the latest-sync summary truthful when the same authenticated connection card has opposite mixed terminal-state rows sharing both newest queued_at and activity timestamps, reusing the same repository id, and the API returns them in reverse order', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-succeeded-newest-shared-repo-exact-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-exact-timestamp-reverse-order',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: null,
          },
          {
            id: 'job-failed-newest-shared-repo-exact-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-exact-timestamp-reverse-order',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-failed-older-shared-repo-exact-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-older-exact-timestamp-reverse-order',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: 'Older mirror fetch failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    const latestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(latestSyncSummary).toHaveTextContent('Latest sync:');
    expect(latestSyncSummary).toHaveTextContent('succeeded');
    expect(latestSyncSummary).toHaveTextContent('repo-shared-newest-exact-timestamp-reverse-order · 2026-04-18T13:00:00Z');
    expect(latestSyncSummary).not.toHaveTextContent('failed');
    expect(latestSyncSummary).not.toHaveTextContent('repo-shared-older-exact-timestamp-reverse-order · 2026-04-18T12:00:00Z');
  });

  it('keeps terminal-state sync-history timestamp details truthful when the same authenticated connection card has opposite mixed terminal-state rows sharing the same newest queued_at timestamp, reusing the same repository id, and the API returns them in reverse order', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-failed-newest-shared-repo-same-timestamp-reverse-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-same-timestamp-reverse-order-details',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-succeeded-newest-shared-repo-same-timestamp-reverse-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-same-timestamp-reverse-order-details',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: null,
          },
          {
            id: 'job-failed-older-shared-repo-same-timestamp-reverse-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-shared-older-reverse-order-details',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: 'Older mirror fetch failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-shared-newest-same-timestamp-reverse-order-details',
      '#/repos/repo-shared-newest-same-timestamp-reverse-order-details',
      '#/repos/repo-shared-older-reverse-order-details',
    ]);

    const terminalRows = within(githubCard!).getAllByRole('link', {
      name: 'Open repository detail for repo-shared-newest-same-timestamp-reverse-order-details',
    });
    expect(terminalRows).toHaveLength(2);

    const newestSucceededRow = terminalRows[0].closest('div');
    expect(newestSucceededRow).toBeInTheDocument();
    expect(within(newestSucceededRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(newestSucceededRow!).getByText('Started at: 2026-04-18T13:02:00Z')).toBeInTheDocument();
    expect(within(newestSucceededRow!).getByText('Finished at: 2026-04-18T13:06:00Z')).toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Started at: 2026-04-18T13:01:00Z')).not.toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Finished at: 2026-04-18T13:05:00Z')).not.toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();

    const newestFailedRow = terminalRows[1].closest('div');
    expect(newestFailedRow).toBeInTheDocument();
    expect(within(newestFailedRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(newestFailedRow!).getByText('Started at: 2026-04-18T13:01:00Z')).toBeInTheDocument();
    expect(within(newestFailedRow!).getByText('Finished at: 2026-04-18T13:05:00Z')).toBeInTheDocument();
    expect(within(newestFailedRow!).queryByText('Started at: 2026-04-18T13:02:00Z')).not.toBeInTheDocument();
    expect(within(newestFailedRow!).queryByText('Finished at: 2026-04-18T13:06:00Z')).not.toBeInTheDocument();
    expect(within(newestFailedRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();

    const olderFailedRow = within(githubCard!).getByRole('link', {
      name: 'Open repository detail for repo-shared-older-reverse-order-details',
    }).closest('div');
    expect(olderFailedRow).toBeInTheDocument();
    expect(within(olderFailedRow!).getByText('Queued at: 2026-04-18T12:00:00Z')).toBeInTheDocument();
    expect(within(olderFailedRow!).getByText('Started at: 2026-04-18T12:02:00Z')).toBeInTheDocument();
    expect(within(olderFailedRow!).getByText('Finished at: 2026-04-18T12:06:00Z')).toBeInTheDocument();
    expect(within(olderFailedRow!).queryByText('Queued at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(olderFailedRow!).queryByText('Started at: 2026-04-18T13:01:00Z')).not.toBeInTheDocument();
    expect(within(olderFailedRow!).queryByText('Started at: 2026-04-18T13:02:00Z')).not.toBeInTheDocument();
    expect(within(olderFailedRow!).queryByText('Finished at: 2026-04-18T13:05:00Z')).not.toBeInTheDocument();
    expect(within(olderFailedRow!).queryByText('Finished at: 2026-04-18T13:06:00Z')).not.toBeInTheDocument();
  });

  it('keeps terminal-state sync-history timestamp details truthful when the same authenticated connection card has opposite mixed terminal-state rows sharing both newest queued_at and activity timestamps, reusing the same repository id, and the API returns them in reverse order', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-failed-newest-shared-repo-exact-timestamp-reverse-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-exact-timestamp-reverse-order-details',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-succeeded-newest-shared-repo-exact-timestamp-reverse-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-exact-timestamp-reverse-order-details',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: null,
          },
          {
            id: 'job-failed-older-shared-repo-exact-timestamp-reverse-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-shared-older-exact-timestamp-reverse-order-details',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: 'Older mirror fetch failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-shared-newest-exact-timestamp-reverse-order-details',
      '#/repos/repo-shared-newest-exact-timestamp-reverse-order-details',
      '#/repos/repo-shared-older-exact-timestamp-reverse-order-details',
    ]);

    const terminalRows = within(githubCard!).getAllByRole('link', {
      name: 'Open repository detail for repo-shared-newest-exact-timestamp-reverse-order-details',
    });
    expect(terminalRows).toHaveLength(2);

    const newestSucceededRow = terminalRows[0].closest('div');
    expect(newestSucceededRow).toBeInTheDocument();
    expect(within(newestSucceededRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(newestSucceededRow!).getByText('Started at: 2026-04-18T13:01:00Z')).toBeInTheDocument();
    expect(within(newestSucceededRow!).getByText('Finished at: 2026-04-18T13:05:00Z')).toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Started at: 2026-04-18T12:02:00Z')).not.toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Finished at: 2026-04-18T12:06:00Z')).not.toBeInTheDocument();

    const newestFailedRow = terminalRows[1].closest('div');
    expect(newestFailedRow).toBeInTheDocument();
    expect(within(newestFailedRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(newestFailedRow!).getByText('Started at: 2026-04-18T13:01:00Z')).toBeInTheDocument();
    expect(within(newestFailedRow!).getByText('Finished at: 2026-04-18T13:05:00Z')).toBeInTheDocument();
    expect(within(newestFailedRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();
    expect(within(newestFailedRow!).queryByText('Started at: 2026-04-18T12:02:00Z')).not.toBeInTheDocument();
    expect(within(newestFailedRow!).queryByText('Finished at: 2026-04-18T12:06:00Z')).not.toBeInTheDocument();

    const olderFailedRow = within(githubCard!).getByRole('link', {
      name: 'Open repository detail for repo-shared-older-exact-timestamp-reverse-order-details',
    }).closest('div');
    expect(olderFailedRow).toBeInTheDocument();
    expect(within(olderFailedRow!).getByText('Queued at: 2026-04-18T12:00:00Z')).toBeInTheDocument();
    expect(within(olderFailedRow!).getByText('Started at: 2026-04-18T12:02:00Z')).toBeInTheDocument();
    expect(within(olderFailedRow!).getByText('Finished at: 2026-04-18T12:06:00Z')).toBeInTheDocument();
    expect(within(olderFailedRow!).queryByText('Queued at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(olderFailedRow!).queryByText('Started at: 2026-04-18T13:01:00Z')).not.toBeInTheDocument();
    expect(within(olderFailedRow!).queryByText('Finished at: 2026-04-18T13:05:00Z')).not.toBeInTheDocument();
  });

  it('keeps terminal-state sync-history timestamp details truthful when the same authenticated connection card has opposite mixed terminal-state rows sharing the same newest queued_at timestamp, already arrive newest-first, and also reuse the same repository id', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-succeeded-newest-shared-repo-same-timestamp-stable-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-same-timestamp-stable-order-details',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: null,
          },
          {
            id: 'job-failed-newest-shared-repo-same-timestamp-stable-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-same-timestamp-stable-order-details',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-failed-older-shared-repo-same-timestamp-stable-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-shared-older-stable-order-details',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: 'Older mirror fetch failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-shared-newest-same-timestamp-stable-order-details',
      '#/repos/repo-shared-newest-same-timestamp-stable-order-details',
      '#/repos/repo-shared-older-stable-order-details',
    ]);

    const terminalRows = within(githubCard!).getAllByRole('link', {
      name: 'Open repository detail for repo-shared-newest-same-timestamp-stable-order-details',
    });
    expect(terminalRows).toHaveLength(2);

    const newestSucceededRow = terminalRows[0].closest('div');
    expect(newestSucceededRow).toBeInTheDocument();
    expect(within(newestSucceededRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(newestSucceededRow!).getByText('Started at: 2026-04-18T13:02:00Z')).toBeInTheDocument();
    expect(within(newestSucceededRow!).getByText('Finished at: 2026-04-18T13:06:00Z')).toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Started at: 2026-04-18T13:01:00Z')).not.toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Finished at: 2026-04-18T13:05:00Z')).not.toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();

    const newestFailedRow = terminalRows[1].closest('div');
    expect(newestFailedRow).toBeInTheDocument();
    expect(within(newestFailedRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(newestFailedRow!).getByText('Started at: 2026-04-18T13:01:00Z')).toBeInTheDocument();
    expect(within(newestFailedRow!).getByText('Finished at: 2026-04-18T13:05:00Z')).toBeInTheDocument();
    expect(within(newestFailedRow!).queryByText('Started at: 2026-04-18T13:02:00Z')).not.toBeInTheDocument();
    expect(within(newestFailedRow!).queryByText('Finished at: 2026-04-18T13:06:00Z')).not.toBeInTheDocument();
    expect(within(newestFailedRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();

    const olderFailedRow = within(githubCard!).getByRole('link', {
      name: 'Open repository detail for repo-shared-older-stable-order-details',
    }).closest('div');
    expect(olderFailedRow).toBeInTheDocument();
    expect(within(olderFailedRow!).getByText('Queued at: 2026-04-18T12:00:00Z')).toBeInTheDocument();
    expect(within(olderFailedRow!).getByText('Started at: 2026-04-18T12:02:00Z')).toBeInTheDocument();
    expect(within(olderFailedRow!).getByText('Finished at: 2026-04-18T12:06:00Z')).toBeInTheDocument();
    expect(within(olderFailedRow!).queryByText('Queued at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(olderFailedRow!).queryByText('Started at: 2026-04-18T13:01:00Z')).not.toBeInTheDocument();
    expect(within(olderFailedRow!).queryByText('Started at: 2026-04-18T13:02:00Z')).not.toBeInTheDocument();
    expect(within(olderFailedRow!).queryByText('Finished at: 2026-04-18T13:05:00Z')).not.toBeInTheDocument();
    expect(within(olderFailedRow!).queryByText('Finished at: 2026-04-18T13:06:00Z')).not.toBeInTheDocument();
  });


  it('keeps terminal-state sync-history timestamp details truthful when the same authenticated connection card has opposite mixed terminal-state rows sharing both newest queued_at and activity timestamps, already arrive newest-first, and also reuse the same repository id', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-succeeded-newest-shared-repo-exact-timestamp-stable-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-exact-timestamp-stable-order-details',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: null,
          },
          {
            id: 'job-failed-newest-shared-repo-exact-timestamp-stable-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-exact-timestamp-stable-order-details',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-failed-older-shared-repo-exact-timestamp-stable-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-shared-older-exact-timestamp-stable-order-details',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: 'Older mirror fetch failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-shared-newest-exact-timestamp-stable-order-details',
      '#/repos/repo-shared-newest-exact-timestamp-stable-order-details',
      '#/repos/repo-shared-older-exact-timestamp-stable-order-details',
    ]);

    const terminalRows = within(githubCard!).getAllByRole('link', {
      name: 'Open repository detail for repo-shared-newest-exact-timestamp-stable-order-details',
    });
    expect(terminalRows).toHaveLength(2);

    const newestSucceededRow = terminalRows[0].closest('div');
    expect(newestSucceededRow).toBeInTheDocument();
    expect(within(newestSucceededRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(newestSucceededRow!).getByText('Started at: 2026-04-18T13:01:00Z')).toBeInTheDocument();
    expect(within(newestSucceededRow!).getByText('Finished at: 2026-04-18T13:05:00Z')).toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Started at: 2026-04-18T12:02:00Z')).not.toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Finished at: 2026-04-18T12:06:00Z')).not.toBeInTheDocument();

    const newestFailedRow = terminalRows[1].closest('div');
    expect(newestFailedRow).toBeInTheDocument();
    expect(within(newestFailedRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(newestFailedRow!).getByText('Started at: 2026-04-18T13:01:00Z')).toBeInTheDocument();
    expect(within(newestFailedRow!).getByText('Finished at: 2026-04-18T13:05:00Z')).toBeInTheDocument();
    expect(within(newestFailedRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();
    expect(within(newestFailedRow!).queryByText('Started at: 2026-04-18T12:02:00Z')).not.toBeInTheDocument();
    expect(within(newestFailedRow!).queryByText('Finished at: 2026-04-18T12:06:00Z')).not.toBeInTheDocument();

    const olderFailedRow = within(githubCard!).getByRole('link', {
      name: 'Open repository detail for repo-shared-older-exact-timestamp-stable-order-details',
    }).closest('div');
    expect(olderFailedRow).toBeInTheDocument();
    expect(within(olderFailedRow!).getByText('Queued at: 2026-04-18T12:00:00Z')).toBeInTheDocument();
    expect(within(olderFailedRow!).getByText('Started at: 2026-04-18T12:02:00Z')).toBeInTheDocument();
    expect(within(olderFailedRow!).getByText('Finished at: 2026-04-18T12:06:00Z')).toBeInTheDocument();
    expect(within(olderFailedRow!).queryByText('Queued at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(olderFailedRow!).queryByText('Started at: 2026-04-18T13:01:00Z')).not.toBeInTheDocument();
    expect(within(olderFailedRow!).queryByText('Finished at: 2026-04-18T13:05:00Z')).not.toBeInTheDocument();
  });

  it('keeps terminal-state sync-history error details truthful when the same authenticated connection card has opposite mixed terminal-state rows sharing the same newest queued_at timestamp, reusing the same repository id, and the API returns them in reverse order', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-failed-newest-shared-repo-same-timestamp-reverse-order-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-same-timestamp-reverse-order-error-details',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-succeeded-newest-shared-repo-same-timestamp-reverse-order-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-same-timestamp-reverse-order-error-details',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: null,
          },
          {
            id: 'job-failed-older-shared-repo-same-timestamp-reverse-order-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-shared-older-reverse-order-error-details',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: 'Older mirror fetch failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-shared-newest-same-timestamp-reverse-order-error-details',
      '#/repos/repo-shared-newest-same-timestamp-reverse-order-error-details',
      '#/repos/repo-shared-older-reverse-order-error-details',
    ]);

    const terminalRows = within(githubCard!).getAllByRole('link', {
      name: 'Open repository detail for repo-shared-newest-same-timestamp-reverse-order-error-details',
    });
    expect(terminalRows).toHaveLength(2);

    const newestSucceededRow = terminalRows[0].closest('div');
    expect(newestSucceededRow).toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText(/Error:/)).not.toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Error: Mirror fetch failed')).not.toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Error: Older mirror fetch failed')).not.toBeInTheDocument();

    const newestFailedRow = terminalRows[1].closest('div');
    expect(newestFailedRow).toBeInTheDocument();
    expect(within(newestFailedRow!).getByText('Error: Mirror fetch failed')).toBeInTheDocument();
    expect(within(newestFailedRow!).queryByText('Error: Older mirror fetch failed')).not.toBeInTheDocument();

    const olderFailedRow = within(githubCard!).getByRole('link', {
      name: 'Open repository detail for repo-shared-older-reverse-order-error-details',
    }).closest('div');
    expect(olderFailedRow).toBeInTheDocument();
    expect(within(olderFailedRow!).getByText('Error: Older mirror fetch failed')).toBeInTheDocument();
    expect(within(olderFailedRow!).queryByText('Error: Mirror fetch failed')).not.toBeInTheDocument();
  });

  it('keeps terminal-state sync-history error details truthful when the same authenticated connection card has opposite mixed terminal-state rows sharing the same newest queued_at timestamp, already arrive newest-first, and also reuse the same repository id', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-succeeded-newest-shared-repo-same-timestamp-stable-order-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-same-timestamp-stable-order-error-details',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: '2026-04-18T13:06:00Z',
            error: null,
          },
          {
            id: 'job-failed-newest-shared-repo-same-timestamp-stable-order-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-same-timestamp-stable-order-error-details',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-failed-older-shared-repo-same-timestamp-stable-order-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-shared-older-stable-order-error-details',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: 'Older mirror fetch failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-shared-newest-same-timestamp-stable-order-error-details',
      '#/repos/repo-shared-newest-same-timestamp-stable-order-error-details',
      '#/repos/repo-shared-older-stable-order-error-details',
    ]);

    const terminalRows = within(githubCard!).getAllByRole('link', {
      name: 'Open repository detail for repo-shared-newest-same-timestamp-stable-order-error-details',
    });
    expect(terminalRows).toHaveLength(2);

    const newestSucceededRow = terminalRows[0].closest('div');
    expect(newestSucceededRow).toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText(/Error:/)).not.toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Error: Mirror fetch failed')).not.toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Error: Older mirror fetch failed')).not.toBeInTheDocument();

    const newestFailedRow = terminalRows[1].closest('div');
    expect(newestFailedRow).toBeInTheDocument();
    expect(within(newestFailedRow!).getByText('Error: Mirror fetch failed')).toBeInTheDocument();
    expect(within(newestFailedRow!).queryByText('Error: Older mirror fetch failed')).not.toBeInTheDocument();

    const olderFailedRow = within(githubCard!).getByRole('link', {
      name: 'Open repository detail for repo-shared-older-stable-order-error-details',
    }).closest('div');
    expect(olderFailedRow).toBeInTheDocument();
    expect(within(olderFailedRow!).getByText('Error: Older mirror fetch failed')).toBeInTheDocument();
    expect(within(olderFailedRow!).queryByText('Error: Mirror fetch failed')).not.toBeInTheDocument();
  });

  it('keeps terminal-state sync-history error details truthful when the same authenticated connection card has opposite mixed terminal-state rows sharing both newest queued_at and activity timestamps, arrive in reverse API order, and also reuse the same repository id', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-failed-newest-shared-repo-exact-timestamp-reverse-order-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-exact-timestamp-reverse-order-error-details',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-succeeded-newest-shared-repo-exact-timestamp-reverse-order-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-exact-timestamp-reverse-order-error-details',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: null,
          },
          {
            id: 'job-failed-older-shared-repo-exact-timestamp-reverse-order-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-shared-older-exact-timestamp-reverse-order-error-details',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: 'Older mirror fetch failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-shared-newest-exact-timestamp-reverse-order-error-details',
      '#/repos/repo-shared-newest-exact-timestamp-reverse-order-error-details',
      '#/repos/repo-shared-older-exact-timestamp-reverse-order-error-details',
    ]);

    const terminalRows = within(githubCard!).getAllByRole('link', {
      name: 'Open repository detail for repo-shared-newest-exact-timestamp-reverse-order-error-details',
    });
    expect(terminalRows).toHaveLength(2);

    const newestSucceededRow = terminalRows[0].closest('div');
    expect(newestSucceededRow).toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText(/Error:/)).not.toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Error: Mirror fetch failed')).not.toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Error: Older mirror fetch failed')).not.toBeInTheDocument();

    const newestFailedRow = terminalRows[1].closest('div');
    expect(newestFailedRow).toBeInTheDocument();
    expect(within(newestFailedRow!).getByText('Error: Mirror fetch failed')).toBeInTheDocument();
    expect(within(newestFailedRow!).queryByText('Error: Older mirror fetch failed')).not.toBeInTheDocument();

    const olderFailedRow = within(githubCard!).getByRole('link', {
      name: 'Open repository detail for repo-shared-older-exact-timestamp-reverse-order-error-details',
    }).closest('div');
    expect(olderFailedRow).toBeInTheDocument();
    expect(within(olderFailedRow!).getByText('Error: Older mirror fetch failed')).toBeInTheDocument();
    expect(within(olderFailedRow!).queryByText('Error: Mirror fetch failed')).not.toBeInTheDocument();
  });

  it('keeps terminal-state sync-history error details truthful when the same authenticated connection card has opposite mixed terminal-state rows sharing both newest queued_at and activity timestamps, already arrive newest-first, and also reuse the same repository id', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-succeeded-newest-shared-repo-exact-timestamp-stable-order-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-exact-timestamp-stable-order-error-details',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: null,
          },
          {
            id: 'job-failed-newest-shared-repo-exact-timestamp-stable-order-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-exact-timestamp-stable-order-error-details',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-failed-older-shared-repo-exact-timestamp-stable-order-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-shared-older-exact-timestamp-stable-order-error-details',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: 'Older mirror fetch failed',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-shared-newest-exact-timestamp-stable-order-error-details',
      '#/repos/repo-shared-newest-exact-timestamp-stable-order-error-details',
      '#/repos/repo-shared-older-exact-timestamp-stable-order-error-details',
    ]);

    const terminalRows = within(githubCard!).getAllByRole('link', {
      name: 'Open repository detail for repo-shared-newest-exact-timestamp-stable-order-error-details',
    });
    expect(terminalRows).toHaveLength(2);

    const newestSucceededRow = terminalRows[0].closest('div');
    expect(newestSucceededRow).toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText(/Error:/)).not.toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Error: Mirror fetch failed')).not.toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Error: Older mirror fetch failed')).not.toBeInTheDocument();

    const newestFailedRow = terminalRows[1].closest('div');
    expect(newestFailedRow).toBeInTheDocument();
    expect(within(newestFailedRow!).getByText('Error: Mirror fetch failed')).toBeInTheDocument();
    expect(within(newestFailedRow!).queryByText('Error: Older mirror fetch failed')).not.toBeInTheDocument();

    const olderFailedRow = within(githubCard!).getByRole('link', {
      name: 'Open repository detail for repo-shared-older-exact-timestamp-stable-order-error-details',
    }).closest('div');
    expect(olderFailedRow).toBeInTheDocument();
    expect(within(olderFailedRow!).getByText('Error: Older mirror fetch failed')).toBeInTheDocument();
    expect(within(olderFailedRow!).queryByText('Error: Mirror fetch failed')).not.toBeInTheDocument();
  });

  it('keeps the latest-sync summary truthful when the same authenticated connection card has an older failed row but a newer succeeded terminal-state row', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-failed-older',
            organization_id: 'org-1',
            repository_id: 'repo-failed-older',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-succeeded-newest',
            organization_id: 'org-1',
            repository_id: 'repo-succeeded-newest',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    const latestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(latestSyncSummary).toHaveTextContent('Latest sync:');
    expect(latestSyncSummary).toHaveTextContent('succeeded');
    expect(latestSyncSummary).toHaveTextContent('repo-succeeded-newest · 2026-04-18T13:00:00Z');
    expect(latestSyncSummary).not.toHaveTextContent('failed');
    expect(latestSyncSummary).not.toHaveTextContent('repo-failed-older · 2026-04-18T12:00:00Z');
  });

  it('keeps terminal-state sync-history error details truthful when the same authenticated connection card has an older failed row but a newer succeeded terminal-state row', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-failed-older-error-details-baseline',
            organization_id: 'org-1',
            repository_id: 'repo-failed-older-error-details-baseline',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-succeeded-newest-error-details-baseline',
            organization_id: 'org-1',
            repository_id: 'repo-succeeded-newest-error-details-baseline',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    const newestSucceededRow = within(githubCard!).getByRole('link', {
      name: 'Open repository detail for repo-succeeded-newest-error-details-baseline',
    }).closest('div');
    expect(newestSucceededRow).toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText(/Error:/)).not.toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Error: Mirror fetch failed')).not.toBeInTheDocument();

    const olderFailedRow = within(githubCard!).getByRole('link', {
      name: 'Open repository detail for repo-failed-older-error-details-baseline',
    }).closest('div');
    expect(olderFailedRow).toBeInTheDocument();
    expect(within(olderFailedRow!).getByText('Error: Mirror fetch failed')).toBeInTheDocument();
  });

  it('keeps terminal-state sync-history timestamp details truthful when the same authenticated connection card has an older failed row but a newer succeeded terminal-state row', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-failed-older-timestamp-details-baseline',
            organization_id: 'org-1',
            repository_id: 'repo-failed-older-timestamp-details-baseline',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:02:00Z',
            finished_at: '2026-04-18T12:06:00Z',
            error: 'Mirror fetch failed',
          },
          {
            id: 'job-succeeded-newest-timestamp-details-baseline',
            organization_id: 'org-1',
            repository_id: 'repo-succeeded-newest-timestamp-details-baseline',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ })).toHaveLength(2);

    const newestSucceededRow = within(githubCard!).getByRole('link', {
      name: 'Open repository detail for repo-succeeded-newest-timestamp-details-baseline',
    }).closest('div');
    expect(newestSucceededRow).toBeInTheDocument();
    expect(within(newestSucceededRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(newestSucceededRow!).getByText('Started at: 2026-04-18T13:01:00Z')).toBeInTheDocument();
    expect(within(newestSucceededRow!).getByText('Finished at: 2026-04-18T13:05:00Z')).toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Started at: 2026-04-18T12:02:00Z')).not.toBeInTheDocument();
    expect(within(newestSucceededRow!).queryByText('Finished at: 2026-04-18T12:06:00Z')).not.toBeInTheDocument();

    const olderFailedRow = within(githubCard!).getByRole('link', {
      name: 'Open repository detail for repo-failed-older-timestamp-details-baseline',
    }).closest('div');
    expect(olderFailedRow).toBeInTheDocument();
    expect(within(olderFailedRow!).getByText('Queued at: 2026-04-18T12:00:00Z')).toBeInTheDocument();
    expect(within(olderFailedRow!).getByText('Started at: 2026-04-18T12:02:00Z')).toBeInTheDocument();
    expect(within(olderFailedRow!).getByText('Finished at: 2026-04-18T12:06:00Z')).toBeInTheDocument();
    expect(within(olderFailedRow!).queryByText('Queued at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(olderFailedRow!).queryByText('Started at: 2026-04-18T13:01:00Z')).not.toBeInTheDocument();
    expect(within(olderFailedRow!).queryByText('Finished at: 2026-04-18T13:05:00Z')).not.toBeInTheDocument();
  });

  it('keeps the latest-sync summary truthful when the same authenticated connection card has an older succeeded row but a newer failed terminal-state row', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-succeeded-older',
            organization_id: 'org-1',
            repository_id: 'repo-succeeded-older',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:01:00Z',
            finished_at: '2026-04-18T12:04:00Z',
            error: null,
          },
          {
            id: 'job-failed-newest',
            organization_id: 'org-1',
            repository_id: 'repo-failed-newest',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'GitHub permissions denied',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    const latestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(latestSyncSummary).toHaveTextContent('Latest sync:');
    expect(latestSyncSummary).toHaveTextContent('failed');
    expect(latestSyncSummary).toHaveTextContent('repo-failed-newest · 2026-04-18T13:00:00Z');
    expect(latestSyncSummary).not.toHaveTextContent('succeeded');
    expect(latestSyncSummary).not.toHaveTextContent('repo-succeeded-older · 2026-04-18T12:00:00Z');
  });

  it('keeps terminal-state sync-history error details truthful when the same authenticated connection card has an older succeeded row but a newer failed terminal-state row', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-succeeded-older-error-details-baseline',
            organization_id: 'org-1',
            repository_id: 'repo-succeeded-older-error-details-baseline',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:01:00Z',
            finished_at: '2026-04-18T12:04:00Z',
            error: null,
          },
          {
            id: 'job-failed-newest-error-details-baseline',
            organization_id: 'org-1',
            repository_id: 'repo-failed-newest-error-details-baseline',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'GitHub permissions denied',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    const newestFailedRow = within(githubCard!).getByRole('link', {
      name: 'Open repository detail for repo-failed-newest-error-details-baseline',
    }).closest('div');
    expect(newestFailedRow).toBeInTheDocument();
    expect(within(newestFailedRow!).getByText('Error: GitHub permissions denied')).toBeInTheDocument();

    const olderSucceededRow = within(githubCard!).getByRole('link', {
      name: 'Open repository detail for repo-succeeded-older-error-details-baseline',
    }).closest('div');
    expect(olderSucceededRow).toBeInTheDocument();
    expect(within(olderSucceededRow!).queryByText(/Error:/)).not.toBeInTheDocument();
    expect(within(olderSucceededRow!).queryByText('Error: GitHub permissions denied')).not.toBeInTheDocument();
  });

  it('keeps terminal-state sync-history timestamp details truthful when the same authenticated connection card has an older succeeded row but a newer failed terminal-state row', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-succeeded-older-timestamp-details-baseline',
            organization_id: 'org-1',
            repository_id: 'repo-succeeded-older-timestamp-details-baseline',
            connection_id: 'conn-1',
            status: 'succeeded',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:01:00Z',
            finished_at: '2026-04-18T12:04:00Z',
            error: null,
          },
          {
            id: 'job-failed-newest-timestamp-details-baseline',
            organization_id: 'org-1',
            repository_id: 'repo-failed-newest-timestamp-details-baseline',
            connection_id: 'conn-1',
            status: 'failed',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: '2026-04-18T13:05:00Z',
            error: 'GitHub permissions denied',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ })).toHaveLength(2);

    const newestFailedRow = within(githubCard!).getByRole('link', {
      name: 'Open repository detail for repo-failed-newest-timestamp-details-baseline',
    }).closest('div');
    expect(newestFailedRow).toBeInTheDocument();
    expect(within(newestFailedRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(newestFailedRow!).getByText('Started at: 2026-04-18T13:01:00Z')).toBeInTheDocument();
    expect(within(newestFailedRow!).getByText('Finished at: 2026-04-18T13:05:00Z')).toBeInTheDocument();
    expect(within(newestFailedRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();
    expect(within(newestFailedRow!).queryByText('Started at: 2026-04-18T12:01:00Z')).not.toBeInTheDocument();
    expect(within(newestFailedRow!).queryByText('Finished at: 2026-04-18T12:04:00Z')).not.toBeInTheDocument();

    const olderSucceededRow = within(githubCard!).getByRole('link', {
      name: 'Open repository detail for repo-succeeded-older-timestamp-details-baseline',
    }).closest('div');
    expect(olderSucceededRow).toBeInTheDocument();
    expect(within(olderSucceededRow!).getByText('Queued at: 2026-04-18T12:00:00Z')).toBeInTheDocument();
    expect(within(olderSucceededRow!).getByText('Started at: 2026-04-18T12:01:00Z')).toBeInTheDocument();
    expect(within(olderSucceededRow!).getByText('Finished at: 2026-04-18T12:04:00Z')).toBeInTheDocument();
    expect(within(olderSucceededRow!).queryByText('Queued at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(olderSucceededRow!).queryByText('Started at: 2026-04-18T13:01:00Z')).not.toBeInTheDocument();
    expect(within(olderSucceededRow!).queryByText('Finished at: 2026-04-18T13:05:00Z')).not.toBeInTheDocument();
  });

  it('renders queued and running sync-history rows with the shared status badge presentation on the settings route', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-running',
            organization_id: 'org-1',
            repository_id: 'repo-running',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:01:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-queued',
            organization_id: 'org-1',
            repository_id: 'repo-queued',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    const latestSyncSummary = within(githubCard!).getByText(/Latest sync:/).closest('div');
    expect(latestSyncSummary).toBeInTheDocument();
    expect(within(latestSyncSummary!).getByText('running')).toBeInTheDocument();
    expect(within(latestSyncSummary!).getByText('repo-running · 2026-04-18T12:00:00Z')).toBeInTheDocument();
    expect(within(latestSyncSummary!).queryByText('Latest sync: running')).not.toBeInTheDocument();

    const runningSyncRow = screen.getByRole('link', { name: 'Open repository detail for repo-running' }).closest('div');
    expect(runningSyncRow).toBeInTheDocument();
    const runningStatusRow = within(runningSyncRow!).getByText('Status').closest('div');
    expect(runningStatusRow).toBeInTheDocument();
    expect(within(runningStatusRow!).getByText('running')).toBeInTheDocument();
    expect(within(runningSyncRow!).queryByText('Status: running')).not.toBeInTheDocument();
    expect(within(runningSyncRow!).getByText('Started at: 2026-04-18T12:01:00Z')).toBeInTheDocument();
    expect(within(runningSyncRow!).getByText('Finished at: Not finished')).toBeInTheDocument();

    const queuedSyncRow = screen.getByRole('link', { name: 'Open repository detail for repo-queued' }).closest('div');
    expect(queuedSyncRow).toBeInTheDocument();
    const queuedStatusRow = within(queuedSyncRow!).getByText('Status').closest('div');
    expect(queuedStatusRow).toBeInTheDocument();
    expect(within(queuedStatusRow!).getByText('queued')).toBeInTheDocument();
    expect(within(queuedSyncRow!).queryByText('Status: queued')).not.toBeInTheDocument();
    expect(within(queuedSyncRow!).getByText('Started at: Not started')).toBeInTheDocument();
    expect(within(queuedSyncRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
  });

  it('keeps multiple queued and running sync-history rows newest-first by queued_at on the settings route', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-running-newest',
            organization_id: 'org-1',
            repository_id: 'repo-running-newest',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-queued-oldest',
            organization_id: 'org-1',
            repository_id: 'repo-queued-oldest',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-running-middle',
            organization_id: 'org-1',
            repository_id: 'repo-running-middle',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:01:00Z',
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    const syncHistoryLinks = within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ });
    expect(syncHistoryLinks.map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-running-newest',
      '#/repos/repo-running-middle',
      '#/repos/repo-queued-oldest',
    ]);
    expect(within(githubCard!).getAllByText(/Queued at:/).map((node) => node.textContent)).toEqual([
      'Queued at: 2026-04-18T13:00:00Z',
      'Queued at: 2026-04-18T12:00:00Z',
      'Queued at: 2026-04-18T11:00:00Z',
    ]);

    const latestSyncSummary = within(githubCard!).getByText(/Latest sync:/).closest('div');
    expect(latestSyncSummary).toBeInTheDocument();
    expect(within(latestSyncSummary!).getByText('running')).toBeInTheDocument();
    expect(within(latestSyncSummary!).getByText('repo-running-newest · 2026-04-18T13:00:00Z')).toBeInTheDocument();
  });

  it('keeps the latest-sync summary truthful when the same authenticated connection card has queued and running rows sharing the same newest queued_at timestamp and the API returns them in reverse order', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-queued-newest-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-queued-newest-same-timestamp',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-running-newest-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-running-newest-same-timestamp',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-running-older-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-running-older',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:01:00Z',
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    const latestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(latestSyncSummary).toHaveTextContent('Latest sync:');
    expect(latestSyncSummary).toHaveTextContent('running');
    expect(latestSyncSummary).toHaveTextContent('repo-running-newest-same-timestamp · 2026-04-18T13:00:00Z');
    expect(latestSyncSummary).not.toHaveTextContent('queued');
    expect(latestSyncSummary).not.toHaveTextContent('repo-queued-newest-same-timestamp · 2026-04-18T13:00:00Z');
    expect(latestSyncSummary).not.toHaveTextContent('repo-running-older · 2026-04-18T12:00:00Z');
  });

  it('keeps the latest-sync summary truthful when the same authenticated connection card has queued and running rows sharing the same newest queued_at and activity timestamps and the API returns them in reverse order', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-queued-newest-identical-activity-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-queued-newest-identical-activity-reverse-order',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-running-newest-identical-activity-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-running-newest-identical-activity-reverse-order',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-running-older-identical-activity-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-running-older-identical-activity-reverse-order',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:05:00Z',
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    const latestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(latestSyncSummary).toHaveTextContent('Latest sync:');
    expect(latestSyncSummary).toHaveTextContent('running');
    expect(latestSyncSummary).toHaveTextContent('repo-running-newest-identical-activity-reverse-order · 2026-04-18T13:00:00Z');
    expect(latestSyncSummary).not.toHaveTextContent('queued');
    expect(latestSyncSummary).not.toHaveTextContent('repo-queued-newest-identical-activity-reverse-order · 2026-04-18T13:00:00Z');
    expect(latestSyncSummary).not.toHaveTextContent('repo-running-older-identical-activity-reverse-order · 2026-04-18T12:00:00Z');
  });

  it('keeps the latest-sync summary truthful when the same authenticated connection card has queued and running rows sharing the same newest queued_at and activity timestamps, reusing the same repository id, and the API returns them in reverse order', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-queued-newest-identical-activity-shared-repo-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-identical-activity-reverse-order',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-running-newest-identical-activity-shared-repo-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-identical-activity-reverse-order',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-running-older-identical-activity-shared-repo-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-older-identical-activity-shared-repo-reverse-order',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:05:00Z',
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    const latestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(latestSyncSummary).toHaveTextContent('Latest sync:');
    expect(latestSyncSummary).toHaveTextContent('running');
    expect(latestSyncSummary).toHaveTextContent('repo-shared-newest-identical-activity-reverse-order · 2026-04-18T13:00:00Z');
    expect(latestSyncSummary).not.toHaveTextContent('queued');
    expect(latestSyncSummary).not.toHaveTextContent('repo-older-identical-activity-shared-repo-reverse-order · 2026-04-18T12:00:00Z');
  });

  it('keeps the latest-sync summary truthful when the same authenticated connection card has queued and running rows tying on both newest queued_at and activity timestamps, already arrive newest-first, and also reuse the same repository id', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-running-newest-identical-activity-shared-repo-stable-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-identical-activity-stable-order',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-queued-newest-identical-activity-shared-repo-stable-order',
            organization_id: 'org-1',
            repository_id: 'repo-shared-newest-identical-activity-stable-order',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-running-older-identical-activity-shared-repo-stable-order',
            organization_id: 'org-1',
            repository_id: 'repo-running-older-identical-activity-shared-repo-stable-order',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:05:00Z',
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    const latestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(latestSyncSummary).toHaveTextContent('Latest sync:');
    expect(latestSyncSummary).toHaveTextContent('running');
    expect(latestSyncSummary).toHaveTextContent('repo-shared-newest-identical-activity-stable-order · 2026-04-18T13:00:00Z');
    expect(latestSyncSummary).not.toHaveTextContent('queued');
    expect(latestSyncSummary).not.toHaveTextContent('repo-running-older-identical-activity-shared-repo-stable-order · 2026-04-18T12:00:00Z');
  });

  it('keeps the latest-sync summary truthful when the same authenticated connection card has queued and running rows sharing the same newest queued_at and activity timestamps', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-queued-newest-identical-activity',
            organization_id: 'org-1',
            repository_id: 'repo-queued-newest-identical-activity',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-running-newest-identical-activity',
            organization_id: 'org-1',
            repository_id: 'repo-running-newest-identical-activity',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-running-older-identical-activity',
            organization_id: 'org-1',
            repository_id: 'repo-running-older-identical-activity',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:05:00Z',
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    const latestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(latestSyncSummary).toHaveTextContent('Latest sync:');
    expect(latestSyncSummary).toHaveTextContent('running');
    expect(latestSyncSummary).toHaveTextContent('repo-running-newest-identical-activity · 2026-04-18T13:00:00Z');
    expect(latestSyncSummary).not.toHaveTextContent('queued');
    expect(latestSyncSummary).not.toHaveTextContent('repo-queued-newest-identical-activity · 2026-04-18T13:00:00Z');
    expect(latestSyncSummary).not.toHaveTextContent('repo-running-older-identical-activity · 2026-04-18T12:00:00Z');
  });

  it('keeps sibling authenticated connection cards latest-sync summaries truthful when queued and running rows share the same newest queued_at timestamp across cards and arrive in reverse API order', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-2-queued-newest-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-queued-newest-same-timestamp',
            connection_id: 'conn-2',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-running-newest-same-timestamp-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-github-running-newest-same-timestamp',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:02:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-running-older-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-running-older',
            connection_id: 'conn-2',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:01:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-queued-older-reverse-order',
            organization_id: 'org-1',
            repository_id: 'repo-github-queued-older',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubLatestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(githubLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(githubLatestSyncSummary).toHaveTextContent('running');
    expect(githubLatestSyncSummary).toHaveTextContent('repo-github-running-newest-same-timestamp · 2026-04-18T13:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('queued');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-github-queued-older · 2026-04-18T11:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-gitlab-queued-newest-same-timestamp · 2026-04-18T13:00:00Z');

    const gitlabLatestSyncSummary = within(gitlabCard!).getByLabelText('Latest sync summary for GitLab Mirror');
    expect(gitlabLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(gitlabLatestSyncSummary).toHaveTextContent('queued');
    expect(gitlabLatestSyncSummary).toHaveTextContent('repo-gitlab-queued-newest-same-timestamp · 2026-04-18T13:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('running');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-gitlab-running-older · 2026-04-18T12:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-github-running-newest-same-timestamp · 2026-04-18T13:00:00Z');
  });

  it('keeps sibling authenticated connection cards latest-sync summaries truthful when queued and running rows share both newest queued_at and activity timestamps across cards', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-2-queued-newest-identical-activity-across-cards',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-queued-newest-identical-activity',
            connection_id: 'conn-2',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-running-newest-identical-activity-across-cards',
            organization_id: 'org-1',
            repository_id: 'repo-github-running-newest-identical-activity',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-running-older-identical-activity-across-cards',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-running-older-identical-activity',
            connection_id: 'conn-2',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:05:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-queued-older-identical-activity-across-cards',
            organization_id: 'org-1',
            repository_id: 'repo-github-queued-older-identical-activity',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubLatestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(githubLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(githubLatestSyncSummary).toHaveTextContent('running');
    expect(githubLatestSyncSummary).toHaveTextContent('repo-github-running-newest-identical-activity · 2026-04-18T13:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('queued');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-github-queued-older-identical-activity · 2026-04-18T12:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-gitlab-queued-newest-identical-activity · 2026-04-18T13:00:00Z');

    const gitlabLatestSyncSummary = within(gitlabCard!).getByLabelText('Latest sync summary for GitLab Mirror');
    expect(gitlabLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(gitlabLatestSyncSummary).toHaveTextContent('queued');
    expect(gitlabLatestSyncSummary).toHaveTextContent('repo-gitlab-queued-newest-identical-activity · 2026-04-18T13:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('running');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-gitlab-running-older-identical-activity · 2026-04-18T12:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-github-running-newest-identical-activity · 2026-04-18T13:00:00Z');
  });

  it('keeps sibling authenticated connection cards latest-sync summaries truthful when queued and running rows share both newest queued_at and activity timestamps across cards and arrive in reverse API order', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-queued-older-identical-activity-reverse-order-across-cards',
            organization_id: 'org-1',
            repository_id: 'repo-github-queued-older-identical-activity-reverse-order',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-running-older-identical-activity-reverse-order-across-cards',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-running-older-identical-activity-reverse-order',
            connection_id: 'conn-2',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:05:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-running-newest-identical-activity-reverse-order-across-cards',
            organization_id: 'org-1',
            repository_id: 'repo-github-running-newest-identical-activity-reverse-order',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-queued-newest-identical-activity-reverse-order-across-cards',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-queued-newest-identical-activity-reverse-order',
            connection_id: 'conn-2',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubLatestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(githubLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(githubLatestSyncSummary).toHaveTextContent('running');
    expect(githubLatestSyncSummary).toHaveTextContent('repo-github-running-newest-identical-activity-reverse-order · 2026-04-18T13:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('queued');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-github-queued-older-identical-activity-reverse-order · 2026-04-18T12:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-gitlab-queued-newest-identical-activity-reverse-order · 2026-04-18T13:00:00Z');

    const gitlabLatestSyncSummary = within(gitlabCard!).getByLabelText('Latest sync summary for GitLab Mirror');
    expect(gitlabLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(gitlabLatestSyncSummary).toHaveTextContent('queued');
    expect(gitlabLatestSyncSummary).toHaveTextContent('repo-gitlab-queued-newest-identical-activity-reverse-order · 2026-04-18T13:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('running');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-gitlab-running-older-identical-activity-reverse-order · 2026-04-18T12:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-github-running-newest-identical-activity-reverse-order · 2026-04-18T13:00:00Z');
  });

  it('keeps sibling authenticated connection cards latest-sync summaries truthful when queued and running rows tie on both newest queued_at and activity timestamps, arrive in reverse API order, and also reuse the same repository id', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-queued-older-shared-repo-identical-activity-reverse-order-across-cards',
            organization_id: 'org-1',
            repository_id: 'repo-github-queued-older-shared-repo-identical-activity-reverse-order',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-running-older-shared-repo-identical-activity-reverse-order-across-cards',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-running-older-shared-repo-identical-activity-reverse-order',
            connection_id: 'conn-2',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:05:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-running-newest-shared-repo-identical-activity-reverse-order-across-cards',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-identical-activity-reverse-order',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-queued-newest-shared-repo-identical-activity-reverse-order-across-cards',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-identical-activity-reverse-order',
            connection_id: 'conn-2',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubLatestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(githubLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(githubLatestSyncSummary).toHaveTextContent('running');
    expect(githubLatestSyncSummary).toHaveTextContent('repo-sibling-shared-newest-identical-activity-reverse-order · 2026-04-18T13:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('queued');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-github-queued-older-shared-repo-identical-activity-reverse-order · 2026-04-18T12:00:00Z');

    const gitlabLatestSyncSummary = within(gitlabCard!).getByLabelText('Latest sync summary for GitLab Mirror');
    expect(gitlabLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(gitlabLatestSyncSummary).toHaveTextContent('queued');
    expect(gitlabLatestSyncSummary).toHaveTextContent('repo-sibling-shared-newest-identical-activity-reverse-order · 2026-04-18T13:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('running');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-gitlab-running-older-shared-repo-identical-activity-reverse-order · 2026-04-18T12:00:00Z');
  });

  it('keeps sibling authenticated connection cards latest-sync summaries truthful when queued and running rows tie on both newest queued_at and activity timestamps, already arrive newest-first, and also reuse the same repository id', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-running-newest-shared-repo-identical-activity-stable-order-across-cards',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-identical-activity-stable-order',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-queued-newest-shared-repo-identical-activity-stable-order-across-cards',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-identical-activity-stable-order',
            connection_id: 'conn-2',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-running-older-shared-repo-identical-activity-stable-order-across-cards',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-running-older-shared-repo-identical-activity-stable-order',
            connection_id: 'conn-2',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:05:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-queued-older-shared-repo-identical-activity-stable-order-across-cards',
            organization_id: 'org-1',
            repository_id: 'repo-github-queued-older-shared-repo-identical-activity-stable-order',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubLatestSyncSummary = within(githubCard!).getByLabelText('Latest sync summary for GitHub Cloud');
    expect(githubLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(githubLatestSyncSummary).toHaveTextContent('running');
    expect(githubLatestSyncSummary).toHaveTextContent('repo-sibling-shared-newest-identical-activity-stable-order · 2026-04-18T13:00:00Z');
    expect(githubLatestSyncSummary).not.toHaveTextContent('queued');
    expect(githubLatestSyncSummary).not.toHaveTextContent('repo-github-queued-older-shared-repo-identical-activity-stable-order · 2026-04-18T12:00:00Z');

    const gitlabLatestSyncSummary = within(gitlabCard!).getByLabelText('Latest sync summary for GitLab Mirror');
    expect(gitlabLatestSyncSummary).toHaveTextContent('Latest sync:');
    expect(gitlabLatestSyncSummary).toHaveTextContent('queued');
    expect(gitlabLatestSyncSummary).toHaveTextContent('repo-sibling-shared-newest-identical-activity-stable-order · 2026-04-18T13:00:00Z');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('running');
    expect(gitlabLatestSyncSummary).not.toHaveTextContent('repo-gitlab-running-older-shared-repo-identical-activity-stable-order · 2026-04-18T12:00:00Z');
  });

  it('keeps multiple queued and running sync-history timestamp details truthful and newest-first on the same authenticated connection card', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-running-newest',
            organization_id: 'org-1',
            repository_id: 'repo-running-newest',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-queued-middle',
            organization_id: 'org-1',
            repository_id: 'repo-queued-middle',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-running-oldest',
            organization_id: 'org-1',
            repository_id: 'repo-running-oldest',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T11:00:00Z',
            started_at: '2026-04-18T11:01:00Z',
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-running-newest',
      '#/repos/repo-queued-middle',
      '#/repos/repo-running-oldest',
    ]);
    expect(within(githubCard!).getAllByText(/Queued at:/).map((node) => node.textContent)).toEqual([
      'Queued at: 2026-04-18T13:00:00Z',
      'Queued at: 2026-04-18T12:00:00Z',
      'Queued at: 2026-04-18T11:00:00Z',
    ]);

    const newestRunningRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-running-newest' }).closest('div');
    expect(newestRunningRow).toBeInTheDocument();
    expect(within(newestRunningRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(newestRunningRow!).getByText('Started at: 2026-04-18T13:01:00Z')).toBeInTheDocument();
    expect(within(newestRunningRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(newestRunningRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();
    expect(within(newestRunningRow!).queryByText('Started at: 2026-04-18T11:01:00Z')).not.toBeInTheDocument();

    const middleQueuedRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-queued-middle' }).closest('div');
    expect(middleQueuedRow).toBeInTheDocument();
    expect(within(middleQueuedRow!).getByText('Queued at: 2026-04-18T12:00:00Z')).toBeInTheDocument();
    expect(within(middleQueuedRow!).getByText('Started at: Not started')).toBeInTheDocument();
    expect(within(middleQueuedRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(middleQueuedRow!).queryByText('Queued at: 2026-04-18T11:00:00Z')).not.toBeInTheDocument();
    expect(within(middleQueuedRow!).queryByText('Started at: 2026-04-18T13:01:00Z')).not.toBeInTheDocument();

    const oldestRunningRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-running-oldest' }).closest('div');
    expect(oldestRunningRow).toBeInTheDocument();
    expect(within(oldestRunningRow!).getByText('Queued at: 2026-04-18T11:00:00Z')).toBeInTheDocument();
    expect(within(oldestRunningRow!).getByText('Started at: 2026-04-18T11:01:00Z')).toBeInTheDocument();
    expect(within(oldestRunningRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(oldestRunningRow!).queryByText('Queued at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(oldestRunningRow!).queryByText('Started at: 2026-04-18T13:01:00Z')).not.toBeInTheDocument();
  });

  it('keeps queued-running sync-history timestamp details deterministic and truthful when rows share both newest queued_at and activity timestamps on the same authenticated connection card and arrive in reverse API order', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-queued-newest-identical-activity-reverse-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-queued-newest-identical-activity-reverse-order-details',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-running-newest-identical-activity-reverse-order-details',
            organization_id: 'org-1',
            repository_id: 'repo-running-newest-identical-activity-reverse-order-details',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-running-older-identical-activity-details',
            organization_id: 'org-1',
            repository_id: 'repo-running-older-identical-activity-details',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:05:00Z',
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-running-newest-identical-activity-reverse-order-details',
      '#/repos/repo-queued-newest-identical-activity-reverse-order-details',
      '#/repos/repo-running-older-identical-activity-details',
    ]);

    const runningNewestRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-running-newest-identical-activity-reverse-order-details' }).closest('div');
    expect(runningNewestRow).toBeInTheDocument();
    expect(within(runningNewestRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(runningNewestRow!).getByText('Started at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(runningNewestRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(runningNewestRow!).queryByText('Started at: Not started')).not.toBeInTheDocument();
    expect(within(runningNewestRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();

    const queuedNewestRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-queued-newest-identical-activity-reverse-order-details' }).closest('div');
    expect(queuedNewestRow).toBeInTheDocument();
    expect(within(queuedNewestRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(queuedNewestRow!).getByText('Started at: Not started')).toBeInTheDocument();
    expect(within(queuedNewestRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(queuedNewestRow!).queryByText('Started at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(queuedNewestRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();
  });

  it('keeps queued-running sync-history timestamp details deterministic and truthful when rows share both newest queued_at and activity timestamps on the same authenticated connection card, already arrive newest-first, and also reuse the same repository id', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-running-newest-identical-activity-stable-order-shared-repo-details',
            organization_id: 'org-1',
            repository_id: 'repo-same-card-shared-newest-identical-activity-stable-order-details',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-queued-newest-identical-activity-stable-order-shared-repo-details',
            organization_id: 'org-1',
            repository_id: 'repo-same-card-shared-newest-identical-activity-stable-order-details',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-running-older-identical-activity-stable-order-shared-repo-details',
            organization_id: 'org-1',
            repository_id: 'repo-running-older-identical-activity-stable-order-shared-repo-details',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:05:00Z',
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-same-card-shared-newest-identical-activity-stable-order-details',
      '#/repos/repo-same-card-shared-newest-identical-activity-stable-order-details',
      '#/repos/repo-running-older-identical-activity-stable-order-shared-repo-details',
    ]);

    const sharedRepoRows = within(githubCard!).getAllByRole('link', {
      name: 'Open repository detail for repo-same-card-shared-newest-identical-activity-stable-order-details',
    });
    expect(sharedRepoRows).toHaveLength(2);

    const runningNewestRow = sharedRepoRows[0].closest('div');
    expect(runningNewestRow).toBeInTheDocument();
    expect(within(runningNewestRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(runningNewestRow!).getByText('Started at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(runningNewestRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(runningNewestRow!).queryByText('Started at: Not started')).not.toBeInTheDocument();
    expect(within(runningNewestRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();

    const queuedNewestRow = sharedRepoRows[1].closest('div');
    expect(queuedNewestRow).toBeInTheDocument();
    expect(within(queuedNewestRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(queuedNewestRow!).getByText('Started at: Not started')).toBeInTheDocument();
    expect(within(queuedNewestRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(queuedNewestRow!).queryByText('Started at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(queuedNewestRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();
  });

  it('keeps queued-running sync-history error details truthful when rows share both newest queued_at and activity timestamps on the same authenticated connection card, arrive in reverse API order, and also reuse the same repository id', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-queued-newest-identical-activity-shared-repo-reverse-order-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-same-card-shared-newest-identical-activity-reverse-order-error-details',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: 'GitHub queue waiting for mirror slot',
          },
          {
            id: 'job-running-newest-identical-activity-shared-repo-reverse-order-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-same-card-shared-newest-identical-activity-reverse-order-error-details',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: 'GitHub fetch still cloning',
          },
          {
            id: 'job-running-older-identical-activity-shared-repo-reverse-order-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-running-older-identical-activity-shared-repo-reverse-order-error-details',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:05:00Z',
            finished_at: null,
            error: 'GitHub older import still running',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-same-card-shared-newest-identical-activity-reverse-order-error-details',
      '#/repos/repo-same-card-shared-newest-identical-activity-reverse-order-error-details',
      '#/repos/repo-running-older-identical-activity-shared-repo-reverse-order-error-details',
    ]);

    const runningNewestRow = within(githubCard!).getByLabelText(
      'Repository sync history row for repo-same-card-shared-newest-identical-activity-reverse-order-error-details (running)',
    );
    expect(runningNewestRow).toBeInTheDocument();
    expect(within(runningNewestRow).getByLabelText(
      'Error details for repo-same-card-shared-newest-identical-activity-reverse-order-error-details (running)',
    )).toHaveTextContent('Error: GitHub fetch still cloning');
    expect(within(runningNewestRow).queryByText('Error: GitHub queue waiting for mirror slot')).not.toBeInTheDocument();
    expect(within(runningNewestRow).queryByText('Error: GitHub older import still running')).not.toBeInTheDocument();

    const queuedNewestRow = within(githubCard!).getByLabelText(
      'Repository sync history row for repo-same-card-shared-newest-identical-activity-reverse-order-error-details (queued)',
    );
    expect(queuedNewestRow).toBeInTheDocument();
    expect(within(queuedNewestRow).getByLabelText(
      'Error details for repo-same-card-shared-newest-identical-activity-reverse-order-error-details (queued)',
    )).toHaveTextContent('Error: GitHub queue waiting for mirror slot');
    expect(within(queuedNewestRow).queryByText('Error: GitHub fetch still cloning')).not.toBeInTheDocument();
    expect(within(queuedNewestRow).queryByText('Error: GitHub older import still running')).not.toBeInTheDocument();

    const olderRunningRow = within(githubCard!).getByLabelText(
      'Repository sync history row for repo-running-older-identical-activity-shared-repo-reverse-order-error-details (running)',
    );
    expect(olderRunningRow).toBeInTheDocument();
    expect(within(olderRunningRow).getByLabelText(
      'Error details for repo-running-older-identical-activity-shared-repo-reverse-order-error-details (running)',
    )).toHaveTextContent('Error: GitHub older import still running');
    expect(within(olderRunningRow).queryByText('Error: GitHub fetch still cloning')).not.toBeInTheDocument();
    expect(within(olderRunningRow).queryByText('Error: GitHub queue waiting for mirror slot')).not.toBeInTheDocument();
  });

  it('keeps queued-running sync-history error details truthful when rows share both newest queued_at and activity timestamps on the same authenticated connection card, already arrive newest-first, and also reuse the same repository id', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-running-newest-identical-activity-shared-repo-stable-order-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-same-card-shared-newest-identical-activity-stable-order-error-details',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: 'GitHub fetch still cloning',
          },
          {
            id: 'job-queued-newest-identical-activity-shared-repo-stable-order-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-same-card-shared-newest-identical-activity-stable-order-error-details',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: 'GitHub queue waiting for mirror slot',
          },
          {
            id: 'job-running-older-identical-activity-shared-repo-stable-order-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-running-older-identical-activity-shared-repo-stable-order-error-details',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:05:00Z',
            finished_at: null,
            error: 'GitHub older import still running',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    expect(githubCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-same-card-shared-newest-identical-activity-stable-order-error-details',
      '#/repos/repo-same-card-shared-newest-identical-activity-stable-order-error-details',
      '#/repos/repo-running-older-identical-activity-shared-repo-stable-order-error-details',
    ]);

    const runningNewestRow = within(githubCard!).getByLabelText(
      'Repository sync history row for repo-same-card-shared-newest-identical-activity-stable-order-error-details (running)',
    );
    expect(runningNewestRow).toBeInTheDocument();
    expect(within(runningNewestRow).getByLabelText(
      'Error details for repo-same-card-shared-newest-identical-activity-stable-order-error-details (running)',
    )).toHaveTextContent('Error: GitHub fetch still cloning');
    expect(within(runningNewestRow).queryByText('Error: GitHub queue waiting for mirror slot')).not.toBeInTheDocument();
    expect(within(runningNewestRow).queryByText('Error: GitHub older import still running')).not.toBeInTheDocument();

    const queuedNewestRow = within(githubCard!).getByLabelText(
      'Repository sync history row for repo-same-card-shared-newest-identical-activity-stable-order-error-details (queued)',
    );
    expect(queuedNewestRow).toBeInTheDocument();
    expect(within(queuedNewestRow).getByLabelText(
      'Error details for repo-same-card-shared-newest-identical-activity-stable-order-error-details (queued)',
    )).toHaveTextContent('Error: GitHub queue waiting for mirror slot');
    expect(within(queuedNewestRow).queryByText('Error: GitHub fetch still cloning')).not.toBeInTheDocument();
    expect(within(queuedNewestRow).queryByText('Error: GitHub older import still running')).not.toBeInTheDocument();

    const olderRunningRow = within(githubCard!).getByLabelText(
      'Repository sync history row for repo-running-older-identical-activity-shared-repo-stable-order-error-details (running)',
    );
    expect(olderRunningRow).toBeInTheDocument();
    expect(within(olderRunningRow).getByLabelText(
      'Error details for repo-running-older-identical-activity-shared-repo-stable-order-error-details (running)',
    )).toHaveTextContent('Error: GitHub older import still running');
    expect(within(olderRunningRow).queryByText('Error: GitHub fetch still cloning')).not.toBeInTheDocument();
    expect(within(olderRunningRow).queryByText('Error: GitHub queue waiting for mirror slot')).not.toBeInTheDocument();
  });

  it('keeps queued and running sync-history rows scoped to their owning connection cards on the settings route', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-running',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-running',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-queued',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-queued',
            connection_id: 'conn-2',
            status: 'queued',
            queued_at: '2026-04-18T12:30:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-queued',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-queued',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-running',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-running',
            connection_id: 'conn-2',
            status: 'running',
            queued_at: '2026-04-18T11:30:00Z',
            started_at: '2026-04-18T11:31:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-succeeded',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-succeeded',
            connection_id: 'conn-2',
            status: 'succeeded',
            queued_at: '2026-04-18T10:30:00Z',
            started_at: '2026-04-18T10:31:00Z',
            finished_at: '2026-04-18T10:33:00Z',
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-conn-1-running',
      '#/repos/repo-conn-1-queued',
    ]);
    expect(within(githubCard!).queryByText('Repository id: repo-conn-2-queued')).not.toBeInTheDocument();
    expect(within(githubCard!).queryByText('Repository id: repo-conn-2-running')).not.toBeInTheDocument();
    const githubLatestSyncSummary = within(githubCard!).getByText(/Latest sync:/).closest('div');
    expect(githubLatestSyncSummary).toBeInTheDocument();
    expect(within(githubLatestSyncSummary!).getByText('repo-conn-1-running · 2026-04-18T13:00:00Z')).toBeInTheDocument();

    expect(within(gitlabCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-conn-2-queued',
      '#/repos/repo-conn-2-running',
      '#/repos/repo-conn-2-succeeded',
    ]);
    expect(within(gitlabCard!).queryByText('Repository id: repo-conn-1-running')).not.toBeInTheDocument();
    expect(within(gitlabCard!).queryByText('Repository id: repo-conn-1-queued')).not.toBeInTheDocument();
    const gitlabLatestSyncSummary = within(gitlabCard!).getByText(/Latest sync:/).closest('div');
    expect(gitlabLatestSyncSummary).toBeInTheDocument();
    expect(within(gitlabLatestSyncSummary!).getByText('repo-conn-2-queued · 2026-04-18T12:30:00Z')).toBeInTheDocument();
    expect(within(gitlabCard!).getAllByText(/Queued at:/).map((node) => node.textContent)).toEqual([
      'Queued at: 2026-04-18T12:30:00Z',
      'Queued at: 2026-04-18T11:30:00Z',
      'Queued at: 2026-04-18T10:30:00Z',
    ]);
  });

  it('keeps in-progress sync-history timestamp details scoped and truthful across authenticated connection cards on the settings route', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-running',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-running',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-queued',
            organization_id: 'org-1',
            repository_id: 'repo-conn-1-queued',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-queued',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-queued',
            connection_id: 'conn-2',
            status: 'queued',
            queued_at: '2026-04-18T11:30:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-running',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-running',
            connection_id: 'conn-2',
            status: 'running',
            queued_at: '2026-04-18T10:30:00Z',
            started_at: '2026-04-18T10:31:00Z',
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubRunningRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-conn-1-running' }).closest('div');
    expect(githubRunningRow).toBeInTheDocument();
    expect(within(githubRunningRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(githubRunningRow!).getByText('Started at: 2026-04-18T13:01:00Z')).toBeInTheDocument();
    expect(within(githubRunningRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(githubRunningRow!).queryByText('Queued at: 2026-04-18T11:30:00Z')).not.toBeInTheDocument();
    expect(within(githubRunningRow!).queryByText('Started at: 2026-04-18T10:31:00Z')).not.toBeInTheDocument();

    const githubQueuedRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-conn-1-queued' }).closest('div');
    expect(githubQueuedRow).toBeInTheDocument();
    expect(within(githubQueuedRow!).getByText('Queued at: 2026-04-18T12:00:00Z')).toBeInTheDocument();
    expect(within(githubQueuedRow!).getByText('Started at: Not started')).toBeInTheDocument();
    expect(within(githubQueuedRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(githubQueuedRow!).queryByText('Queued at: 2026-04-18T10:30:00Z')).not.toBeInTheDocument();
    expect(within(githubQueuedRow!).queryByText('Started at: 2026-04-18T13:01:00Z')).not.toBeInTheDocument();

    const gitlabQueuedRow = within(gitlabCard!).getByRole('link', { name: 'Open repository detail for repo-conn-2-queued' }).closest('div');
    expect(gitlabQueuedRow).toBeInTheDocument();
    expect(within(gitlabQueuedRow!).getByText('Queued at: 2026-04-18T11:30:00Z')).toBeInTheDocument();
    expect(within(gitlabQueuedRow!).getByText('Started at: Not started')).toBeInTheDocument();
    expect(within(gitlabQueuedRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(gitlabQueuedRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();
    expect(within(gitlabQueuedRow!).queryByText('Started at: 2026-04-18T13:01:00Z')).not.toBeInTheDocument();

    const gitlabRunningRow = within(gitlabCard!).getByRole('link', { name: 'Open repository detail for repo-conn-2-running' }).closest('div');
    expect(gitlabRunningRow).toBeInTheDocument();
    expect(within(gitlabRunningRow!).getByText('Queued at: 2026-04-18T10:30:00Z')).toBeInTheDocument();
    expect(within(gitlabRunningRow!).getByText('Started at: 2026-04-18T10:31:00Z')).toBeInTheDocument();
    expect(within(gitlabRunningRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(gitlabRunningRow!).queryByText('Queued at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(gitlabRunningRow!).queryByText('Started at: 2026-04-18T13:01:00Z')).not.toBeInTheDocument();
  });

  it('keeps sibling authenticated connection cards queued-running sync-history timestamp details deterministic and truthful when rows share both newest queued_at and activity timestamps and arrive in reverse API order', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-2-queued-newest-identical-activity-reverse-order-across-cards-details',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-queued-newest-identical-activity-reverse-order-details',
            connection_id: 'conn-2',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-running-newest-identical-activity-reverse-order-across-cards-details',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-running-newest-identical-activity-reverse-order-details',
            connection_id: 'conn-2',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-queued-newest-identical-activity-reverse-order-across-cards-details',
            organization_id: 'org-1',
            repository_id: 'repo-github-queued-newest-identical-activity-reverse-order-details',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-running-newest-identical-activity-reverse-order-across-cards-details',
            organization_id: 'org-1',
            repository_id: 'repo-github-running-newest-identical-activity-reverse-order-details',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-running-older',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-running-older',
            connection_id: 'conn-2',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:05:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-queued-older',
            organization_id: 'org-1',
            repository_id: 'repo-github-queued-older',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-github-running-newest-identical-activity-reverse-order-details',
      '#/repos/repo-github-queued-newest-identical-activity-reverse-order-details',
      '#/repos/repo-github-queued-older',
    ]);
    expect(within(gitlabCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-gitlab-running-newest-identical-activity-reverse-order-details',
      '#/repos/repo-gitlab-queued-newest-identical-activity-reverse-order-details',
      '#/repos/repo-gitlab-running-older',
    ]);

    const githubRunningRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-github-running-newest-identical-activity-reverse-order-details' }).closest('div');
    expect(githubRunningRow).toBeInTheDocument();
    expect(within(githubRunningRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(githubRunningRow!).getByText('Started at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(githubRunningRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(githubRunningRow!).queryByText('Started at: Not started')).not.toBeInTheDocument();
    expect(within(githubRunningRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();

    const githubQueuedRow = within(githubCard!).getByRole('link', { name: 'Open repository detail for repo-github-queued-newest-identical-activity-reverse-order-details' }).closest('div');
    expect(githubQueuedRow).toBeInTheDocument();
    expect(within(githubQueuedRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(githubQueuedRow!).getByText('Started at: Not started')).toBeInTheDocument();
    expect(within(githubQueuedRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(githubQueuedRow!).queryByText('Started at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(githubQueuedRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();

    const gitlabRunningRow = within(gitlabCard!).getByRole('link', { name: 'Open repository detail for repo-gitlab-running-newest-identical-activity-reverse-order-details' }).closest('div');
    expect(gitlabRunningRow).toBeInTheDocument();
    expect(within(gitlabRunningRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(gitlabRunningRow!).getByText('Started at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(gitlabRunningRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(gitlabRunningRow!).queryByText('Started at: Not started')).not.toBeInTheDocument();
    expect(within(gitlabRunningRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();

    const gitlabQueuedRow = within(gitlabCard!).getByRole('link', { name: 'Open repository detail for repo-gitlab-queued-newest-identical-activity-reverse-order-details' }).closest('div');
    expect(gitlabQueuedRow).toBeInTheDocument();
    expect(within(gitlabQueuedRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(gitlabQueuedRow!).getByText('Started at: Not started')).toBeInTheDocument();
    expect(within(gitlabQueuedRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(gitlabQueuedRow!).queryByText('Started at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(gitlabQueuedRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();
  });

  it('keeps sibling authenticated connection cards queued-running sync-history timestamp details deterministic and truthful when rows share both newest queued_at and activity timestamps, arrive in reverse API order, and also reuse the same repository id', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-2-queued-newest-identical-activity-reverse-order-across-cards-shared-repo-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-identical-activity-reverse-order-details',
            connection_id: 'conn-2',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-running-newest-identical-activity-reverse-order-across-cards-shared-repo-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-identical-activity-reverse-order-details',
            connection_id: 'conn-2',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-queued-newest-identical-activity-reverse-order-across-cards-shared-repo-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-identical-activity-reverse-order-details',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-running-newest-identical-activity-reverse-order-across-cards-shared-repo-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-identical-activity-reverse-order-details',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-running-older-shared-repo-details',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-running-older-shared-repo-details',
            connection_id: 'conn-2',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:05:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-queued-older-shared-repo-details',
            organization_id: 'org-1',
            repository_id: 'repo-github-queued-older-shared-repo-details',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-sibling-shared-newest-identical-activity-reverse-order-details',
      '#/repos/repo-sibling-shared-newest-identical-activity-reverse-order-details',
      '#/repos/repo-github-queued-older-shared-repo-details',
    ]);
    expect(within(gitlabCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-sibling-shared-newest-identical-activity-reverse-order-details',
      '#/repos/repo-sibling-shared-newest-identical-activity-reverse-order-details',
      '#/repos/repo-gitlab-running-older-shared-repo-details',
    ]);

    const githubSharedRepoRows = within(githubCard!).getAllByRole('link', { name: 'Open repository detail for repo-sibling-shared-newest-identical-activity-reverse-order-details' });
    expect(githubSharedRepoRows).toHaveLength(2);
    const githubRunningRow = githubSharedRepoRows[0].closest('div');
    expect(githubRunningRow).toBeInTheDocument();
    expect(within(githubRunningRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(githubRunningRow!).getByText('Started at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(githubRunningRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(githubRunningRow!).queryByText('Started at: Not started')).not.toBeInTheDocument();
    expect(within(githubRunningRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();

    const githubQueuedRow = githubSharedRepoRows[1].closest('div');
    expect(githubQueuedRow).toBeInTheDocument();
    expect(within(githubQueuedRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(githubQueuedRow!).getByText('Started at: Not started')).toBeInTheDocument();
    expect(within(githubQueuedRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(githubQueuedRow!).queryByText('Started at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(githubQueuedRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();

    const gitlabSharedRepoRows = within(gitlabCard!).getAllByRole('link', { name: 'Open repository detail for repo-sibling-shared-newest-identical-activity-reverse-order-details' });
    expect(gitlabSharedRepoRows).toHaveLength(2);
    const gitlabRunningRow = gitlabSharedRepoRows[0].closest('div');
    expect(gitlabRunningRow).toBeInTheDocument();
    expect(within(gitlabRunningRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(gitlabRunningRow!).getByText('Started at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(gitlabRunningRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(gitlabRunningRow!).queryByText('Started at: Not started')).not.toBeInTheDocument();
    expect(within(gitlabRunningRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();

    const gitlabQueuedRow = gitlabSharedRepoRows[1].closest('div');
    expect(gitlabQueuedRow).toBeInTheDocument();
    expect(within(gitlabQueuedRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(gitlabQueuedRow!).getByText('Started at: Not started')).toBeInTheDocument();
    expect(within(gitlabQueuedRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(gitlabQueuedRow!).queryByText('Started at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(gitlabQueuedRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();
  });

  it('keeps sibling authenticated connection cards queued-running sync-history timestamp details deterministic and truthful when rows share both newest queued_at and activity timestamps, already arrive newest-first, and also reuse the same repository id', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-running-newest-identical-activity-stable-order-across-cards-shared-repo-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-identical-activity-stable-order-details',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-queued-newest-identical-activity-stable-order-across-cards-shared-repo-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-identical-activity-stable-order-details',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-running-newest-identical-activity-stable-order-across-cards-shared-repo-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-identical-activity-stable-order-details',
            connection_id: 'conn-2',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-queued-newest-identical-activity-stable-order-across-cards-shared-repo-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-identical-activity-stable-order-details',
            connection_id: 'conn-2',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-1-queued-older-stable-order-shared-repo-details',
            organization_id: 'org-1',
            repository_id: 'repo-github-queued-older-stable-order-shared-repo-details',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-running-older-stable-order-shared-repo-details',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-running-older-stable-order-shared-repo-details',
            connection_id: 'conn-2',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:05:00Z',
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    expect(within(githubCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-sibling-shared-newest-identical-activity-stable-order-details',
      '#/repos/repo-sibling-shared-newest-identical-activity-stable-order-details',
      '#/repos/repo-github-queued-older-stable-order-shared-repo-details',
    ]);
    expect(within(gitlabCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-sibling-shared-newest-identical-activity-stable-order-details',
      '#/repos/repo-sibling-shared-newest-identical-activity-stable-order-details',
      '#/repos/repo-gitlab-running-older-stable-order-shared-repo-details',
    ]);

    const githubSharedRepoRows = within(githubCard!).getAllByRole('link', { name: 'Open repository detail for repo-sibling-shared-newest-identical-activity-stable-order-details' });
    expect(githubSharedRepoRows).toHaveLength(2);
    const githubRunningRow = githubSharedRepoRows[0].closest('div');
    expect(githubRunningRow).toBeInTheDocument();
    expect(within(githubRunningRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(githubRunningRow!).getByText('Started at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(githubRunningRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(githubRunningRow!).queryByText('Started at: Not started')).not.toBeInTheDocument();
    expect(within(githubRunningRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();

    const githubQueuedRow = githubSharedRepoRows[1].closest('div');
    expect(githubQueuedRow).toBeInTheDocument();
    expect(within(githubQueuedRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(githubQueuedRow!).getByText('Started at: Not started')).toBeInTheDocument();
    expect(within(githubQueuedRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(githubQueuedRow!).queryByText('Started at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(githubQueuedRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();

    const gitlabSharedRepoRows = within(gitlabCard!).getAllByRole('link', { name: 'Open repository detail for repo-sibling-shared-newest-identical-activity-stable-order-details' });
    expect(gitlabSharedRepoRows).toHaveLength(2);
    const gitlabRunningRow = gitlabSharedRepoRows[0].closest('div');
    expect(gitlabRunningRow).toBeInTheDocument();
    expect(within(gitlabRunningRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(gitlabRunningRow!).getByText('Started at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(gitlabRunningRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(gitlabRunningRow!).queryByText('Started at: Not started')).not.toBeInTheDocument();
    expect(within(gitlabRunningRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();

    const gitlabQueuedRow = gitlabSharedRepoRows[1].closest('div');
    expect(gitlabQueuedRow).toBeInTheDocument();
    expect(within(gitlabQueuedRow!).getByText('Queued at: 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(gitlabQueuedRow!).getByText('Started at: Not started')).toBeInTheDocument();
    expect(within(gitlabQueuedRow!).getByText('Finished at: Not finished')).toBeInTheDocument();
    expect(within(gitlabQueuedRow!).queryByText('Started at: 2026-04-18T13:00:00Z')).not.toBeInTheDocument();
    expect(within(gitlabQueuedRow!).queryByText('Queued at: 2026-04-18T12:00:00Z')).not.toBeInTheDocument();
  });

  it('keeps sibling authenticated connection cards queued-running sync-history error details truthful when rows share both newest queued_at and activity timestamps, already arrive newest-first, and also reuse the same repository id', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-1-running-newest-identical-activity-stable-order-across-cards-shared-repo-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-identical-activity-stable-order-error-details',
            connection_id: 'conn-1',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: 'GitHub fetch still cloning',
          },
          {
            id: 'job-conn-1-queued-newest-identical-activity-stable-order-across-cards-shared-repo-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-identical-activity-stable-order-error-details',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: 'GitHub queue waiting for mirror slot',
          },
          {
            id: 'job-conn-2-running-newest-identical-activity-stable-order-across-cards-shared-repo-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-identical-activity-stable-order-error-details',
            connection_id: 'conn-2',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:00:00Z',
            finished_at: null,
            error: 'GitLab import still running',
          },
          {
            id: 'job-conn-2-queued-newest-identical-activity-stable-order-across-cards-shared-repo-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-sibling-shared-newest-identical-activity-stable-order-error-details',
            connection_id: 'conn-2',
            status: 'queued',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: null,
            finished_at: null,
            error: 'GitLab queue waiting for credentials',
          },
          {
            id: 'job-conn-1-queued-older-stable-order-shared-repo-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-github-queued-older-stable-order-error-details',
            connection_id: 'conn-1',
            status: 'queued',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: null,
            finished_at: null,
            error: 'GitHub older queue waiting for mirror slot',
          },
          {
            id: 'job-conn-2-running-older-stable-order-shared-repo-error-details',
            organization_id: 'org-1',
            repository_id: 'repo-gitlab-running-older-stable-order-error-details',
            connection_id: 'conn-2',
            status: 'running',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: '2026-04-18T12:05:00Z',
            finished_at: null,
            error: 'GitLab older import still running',
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    const githubRunningRow = within(githubCard!).getByLabelText(
      'Repository sync history row for repo-sibling-shared-newest-identical-activity-stable-order-error-details (running)',
    );
    expect(githubRunningRow).toBeInTheDocument();
    expect(within(githubRunningRow).getByLabelText(
      'Error details for repo-sibling-shared-newest-identical-activity-stable-order-error-details (running)',
    )).toHaveTextContent('Error: GitHub fetch still cloning');
    expect(within(githubRunningRow).queryByText('Error: GitLab import still running')).not.toBeInTheDocument();
    expect(within(githubRunningRow).queryByText('Error: GitHub queue waiting for mirror slot')).not.toBeInTheDocument();

    const githubQueuedRow = within(githubCard!).getByLabelText(
      'Repository sync history row for repo-sibling-shared-newest-identical-activity-stable-order-error-details (queued)',
    );
    expect(githubQueuedRow).toBeInTheDocument();
    expect(within(githubQueuedRow).getByLabelText(
      'Error details for repo-sibling-shared-newest-identical-activity-stable-order-error-details (queued)',
    )).toHaveTextContent('Error: GitHub queue waiting for mirror slot');
    expect(within(githubQueuedRow).queryByText('Error: GitHub fetch still cloning')).not.toBeInTheDocument();
    expect(within(githubQueuedRow).queryByText('Error: GitLab queue waiting for credentials')).not.toBeInTheDocument();

    const gitlabRunningRow = within(gitlabCard!).getByLabelText(
      'Repository sync history row for repo-sibling-shared-newest-identical-activity-stable-order-error-details (running)',
    );
    expect(gitlabRunningRow).toBeInTheDocument();
    expect(within(gitlabRunningRow).getByLabelText(
      'Error details for repo-sibling-shared-newest-identical-activity-stable-order-error-details (running)',
    )).toHaveTextContent('Error: GitLab import still running');
    expect(within(gitlabRunningRow).queryByText('Error: GitHub fetch still cloning')).not.toBeInTheDocument();
    expect(within(gitlabRunningRow).queryByText('Error: GitLab queue waiting for credentials')).not.toBeInTheDocument();

    const gitlabQueuedRow = within(gitlabCard!).getByLabelText(
      'Repository sync history row for repo-sibling-shared-newest-identical-activity-stable-order-error-details (queued)',
    );
    expect(gitlabQueuedRow).toBeInTheDocument();
    expect(within(gitlabQueuedRow).getByLabelText(
      'Error details for repo-sibling-shared-newest-identical-activity-stable-order-error-details (queued)',
    )).toHaveTextContent('Error: GitLab queue waiting for credentials');
    expect(within(gitlabQueuedRow).queryByText('Error: GitHub queue waiting for mirror slot')).not.toBeInTheDocument();
    expect(within(gitlabQueuedRow).queryByText('Error: GitLab import still running')).not.toBeInTheDocument();
  });

  it('keeps the empty sync-history state on one authenticated connection card while another shows queued and running rows on the settings route', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([
          {
            id: 'job-conn-2-running',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-running',
            connection_id: 'conn-2',
            status: 'running',
            queued_at: '2026-04-18T13:00:00Z',
            started_at: '2026-04-18T13:01:00Z',
            finished_at: null,
            error: null,
          },
          {
            id: 'job-conn-2-queued',
            organization_id: 'org-1',
            repository_id: 'repo-conn-2-queued',
            connection_id: 'conn-2',
            status: 'queued',
            queued_at: '2026-04-18T12:00:00Z',
            started_at: null,
            finished_at: null,
            error: null,
          },
        ]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    expect(within(githubCard!).getByText('No repository sync jobs found for this connection.')).toBeInTheDocument();
    expect(within(githubCard!).queryByText(/Latest sync:/)).not.toBeInTheDocument();
    expect(within(githubCard!).queryByRole('link', { name: /Open repository detail for repo-/ })).not.toBeInTheDocument();

    const gitlabLatestSyncSummary = within(gitlabCard!).getByText(/Latest sync:/).closest('div');
    expect(gitlabLatestSyncSummary).toBeInTheDocument();
    expect(within(gitlabLatestSyncSummary!).getByText('running')).toBeInTheDocument();
    expect(within(gitlabLatestSyncSummary!).getByText('repo-conn-2-running · 2026-04-18T13:00:00Z')).toBeInTheDocument();
    expect(within(gitlabCard!).queryByText('No repository sync jobs found for this connection.')).not.toBeInTheDocument();
    expect(within(gitlabCard!).getAllByRole('link', { name: /Open repository detail for repo-/ }).map((link) => link.getAttribute('href'))).toEqual([
      '#/repos/repo-conn-2-running',
      '#/repos/repo-conn-2-queued',
    ]);
    expect(within(gitlabCard!).getAllByText(/Queued at:/).map((node) => node.textContent)).toEqual([
      'Queued at: 2026-04-18T13:00:00Z',
      'Queued at: 2026-04-18T12:00:00Z',
    ]);
  });

  it('keeps the sync-history loading state visible and truthful on each authenticated connection card before sync jobs resolve', async () => {
    window.location.hash = '#/settings/connections';

    const deferredSyncJobs = deferredResponse();
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitLab Mirror',
            kind: 'gitlab',
            config: {
              provider: 'gitlab',
              base_url: 'https://gitlab.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return await deferredSyncJobs.promise;
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const gitlabCard = screen.getByText('GitLab Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(gitlabCard).toBeInTheDocument();

    expect(within(githubCard!).getByText('Loading repository sync history…')).toBeInTheDocument();
    expect(within(githubCard!).queryByText('No repository sync jobs found for this connection.')).not.toBeInTheDocument();
    expect(within(githubCard!).queryByText(/Latest sync:/)).not.toBeInTheDocument();
    expect(within(githubCard!).queryByRole('link', { name: /Open repository detail for repo-/ })).not.toBeInTheDocument();

    expect(within(gitlabCard!).getByText('Loading repository sync history…')).toBeInTheDocument();
    expect(within(gitlabCard!).queryByText('No repository sync jobs found for this connection.')).not.toBeInTheDocument();
    expect(within(gitlabCard!).queryByText(/Latest sync:/)).not.toBeInTheDocument();
    expect(within(gitlabCard!).queryByRole('link', { name: /Open repository detail for repo-/ })).not.toBeInTheDocument();

    deferredSyncJobs.resolve(jsonResponse([]));
    expect(await screen.findAllByText('No repository sync jobs found for this connection.')).toHaveLength(2);
  });

  it('keeps the sync-history failure state visible and truthful on each authenticated connection card without collapsing sibling cards', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'Generic Mirror',
            kind: 'generic_git',
            config: {
              provider: 'generic_git',
              base_url: 'https://git.internal.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse({}, false, 503);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('GitHub Cloud')).toBeInTheDocument();

    const expectedError = 'Failed to load repository sync history: Request failed: 503';
    expect(await screen.findAllByText(expectedError)).toHaveLength(2);

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const genericCard = screen.getByText('Generic Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(genericCard).toBeInTheDocument();

    for (const card of [githubCard, genericCard]) {
      expect(card).not.toBeNull();
      expect(within(card!).getByText(expectedError)).toBeInTheDocument();
      expect(within(card!).queryByText('Loading repository sync history…')).not.toBeInTheDocument();
      expect(within(card!).queryByText('No repository sync jobs found for this connection.')).not.toBeInTheDocument();
      expect(within(card!).queryByText(/Latest sync:/)).not.toBeInTheDocument();
      expect(within(card!).queryByRole('link', { name: /Open repository detail for repo-/ })).not.toBeInTheDocument();
    }

    expect(within(githubCard!).getByText('Base URL: https://github.com')).toBeInTheDocument();
    expect(within(genericCard!).getByText('Base URL: https://git.internal.example.com')).toBeInTheDocument();
    expect(within(genericCard!).getByText('Repository discovery is not available yet for generic Git connections.')).toBeInTheDocument();
  });

  it('shows a generic git quick-open affordance on the settings route while keeping discovery status truthful', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'Generic Mirror',
            kind: 'generic_git',
            config: {
              provider: 'generic_git',
              base_url: 'https://git.internal.example.com',
            },
          },
          {
            id: 'conn-3',
            name: 'Unsafe Generic Mirror',
            kind: 'generic_git',
            config: {
              provider: 'generic_git',
              base_url: 'javascript:alert(1)',
            },
          },
          {
            id: 'conn-4',
            name: 'Local Mirror',
            kind: 'local',
            config: {
              provider: 'local',
              repo_path: '/srv/git/mirror',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const genericCard = screen.getByText('Generic Mirror').closest('article');
    const unsafeGenericCard = screen.getByText('Unsafe Generic Mirror').closest('article');
    const localCard = screen.getByText('Local Mirror').closest('article');

    expect(githubCard).toBeInTheDocument();
    expect(genericCard).toBeInTheDocument();
    expect(unsafeGenericCard).toBeInTheDocument();
    expect(localCard).toBeInTheDocument();

    expect(within(genericCard!).getByText('Discovery status')).toBeInTheDocument();
    expect(
      within(genericCard!).getByText('Repository discovery is not available yet for generic Git connections.')
    ).toBeInTheDocument();
    const genericQuickOpenLink = within(genericCard!).getByRole('link', { name: 'Open host for manual discovery' });
    expect(genericQuickOpenLink).toHaveAttribute('href', 'https://git.internal.example.com/');
    expect(genericQuickOpenLink).toHaveAttribute('target', '_blank');
    expect(genericQuickOpenLink).toHaveAttribute('rel', 'noreferrer');
    expect(within(unsafeGenericCard!).getByText('Discovery status')).toBeInTheDocument();
    expect(
      within(unsafeGenericCard!).getByText('Repository discovery is not available yet for generic Git connections.')
    ).toBeInTheDocument();
    expect(within(unsafeGenericCard!).queryByRole('link', { name: 'Open host for manual discovery' })).not.toBeInTheDocument();
    expect(within(localCard!).getByText('Discovery status')).toBeInTheDocument();
    expect(
      within(localCard!).getByText('Import one repository path at a time from this local root.')
    ).toBeInTheDocument();
    expect(
      within(localCard!).getByText('Recursive local enumeration is not available yet.')
    ).toBeInTheDocument();
    expect(within(localCard!).queryByRole('link', { name: 'Open host for manual discovery' })).not.toBeInTheDocument();
    expect(within(githubCard!).queryByText('Discovery status')).not.toBeInTheDocument();
  });

  it('shows a local-only repository import form on the settings route and renders imported repository details on success', async () => {
    window.location.hash = '#/settings/connections';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'Local Mirror',
            kind: 'local',
            config: {
              provider: 'local',
              repo_path: '/srv/git/mirror',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/repositories/import/local' && init?.method === 'POST') {
        expect(init.headers).toEqual({ 'Content-Type': 'application/json' });
        expect(init.body).toBe(
          JSON.stringify({
            connection_id: 'conn-2',
            path: '/srv/git/mirror/project-alpha',
          })
        );

        return jsonResponse({
          repository: {
            id: 'repo-77',
            name: 'project-alpha',
            default_branch: 'main',
            connection_id: 'conn-2',
            sync_state: 'ready',
          },
          connection: {
            id: 'conn-2',
            name: 'Local Mirror',
            kind: 'local',
          },
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const localCard = screen.getByText('Local Mirror').closest('article');

    expect(githubCard).toBeInTheDocument();
    expect(localCard).toBeInTheDocument();
    expect(within(githubCard!).queryByLabelText('Repository path')).not.toBeInTheDocument();
    expect(within(localCard!).getByLabelText('Repository path')).toBeInTheDocument();

    fireEvent.change(within(localCard!).getByLabelText('Repository path'), {
      target: { value: '/srv/git/mirror/project-alpha' },
    });
    fireEvent.click(within(localCard!).getByRole('button', { name: 'Import repository' }));

    expect(await within(localCard!).findByText('Imported repository: project-alpha')).toBeInTheDocument();
    expect(within(localCard!).getByText('Repository id: repo-77')).toBeInTheDocument();
    expect(within(localCard!).getByText('Default branch: main')).toBeInTheDocument();
    expect(within(localCard!).getByText('Sync state')).toBeInTheDocument();
    expect(within(localCard!).getByText('ready')).toBeInTheDocument();
    expect(within(localCard!).queryByText('Sync state: ready')).not.toBeInTheDocument();
    expect(within(localCard!).getByText('Kind: local')).toBeInTheDocument();
    expect(within(localCard!).queryByText(/Failed to import repository:/i)).not.toBeInTheDocument();
    expect(within(localCard!).getByRole('link', { name: 'Open repository detail' })).toHaveAttribute(
      'href',
      '#/repos/repo-77'
    );

    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/connections');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/repository-sync-jobs');
      expect(fetchMock).toHaveBeenNthCalledWith(3, '/api/v1/auth/repositories/import/local', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          connection_id: 'conn-2',
          path: '/srv/git/mirror/project-alpha',
        }),
      });
    });
  });

  it('clears stale imported repository details after the local import path changes', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-2',
            name: 'Local Mirror',
            kind: 'local',
            config: {
              provider: 'local',
              repo_path: '/srv/git/mirror',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/repositories/import/local' && init?.method === 'POST') {
        return jsonResponse({
          repository: {
            id: 'repo-77',
            name: 'project-alpha',
            default_branch: 'main',
            connection_id: 'conn-2',
            sync_state: 'ready',
          },
          connection: {
            id: 'conn-2',
            name: 'Local Mirror',
            kind: 'local',
          },
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const localCard = screen.getByText('Local Mirror').closest('article');
    expect(localCard).toBeInTheDocument();

    fireEvent.change(within(localCard!).getByLabelText('Repository path'), {
      target: { value: '/srv/git/mirror/project-alpha' },
    });
    fireEvent.click(within(localCard!).getByRole('button', { name: 'Import repository' }));

    expect(await within(localCard!).findByText('Imported repository: project-alpha')).toBeInTheDocument();
    expect(within(localCard!).getByText('Repository id: repo-77')).toBeInTheDocument();

    fireEvent.change(within(localCard!).getByLabelText('Repository path'), {
      target: { value: '/srv/git/mirror/project-beta' },
    });

    expect(within(localCard!).queryByText('Imported repository: project-alpha')).not.toBeInTheDocument();
    expect(within(localCard!).queryByText('Repository id: repo-77')).not.toBeInTheDocument();
    expect(within(localCard!).queryByRole('link', { name: 'Open repository detail' })).not.toBeInTheDocument();
  });

  it('restores the local import path to the configured connection root', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-2',
            name: 'Local Mirror',
            kind: 'local',
            config: {
              provider: 'local',
              repo_path: '/srv/git/mirror',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/repositories/import/local' && init?.method === 'POST') {
        return jsonResponse({
          repository: {
            id: 'repo-77',
            name: 'project-alpha',
            default_branch: 'main',
            connection_id: 'conn-2',
            sync_state: 'ready',
          },
          connection: {
            id: 'conn-2',
            name: 'Local Mirror',
            kind: 'local',
          },
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const localCard = screen.getByText('Local Mirror').closest('article');
    expect(localCard).toBeInTheDocument();
    expect(within(localCard!).queryByRole('button', { name: 'Reset to local root' })).not.toBeInTheDocument();

    const pathInput = within(localCard!).getByLabelText('Repository path') as HTMLInputElement;
    fireEvent.change(pathInput, {
      target: { value: '/srv/git/mirror/project-alpha' },
    });
    fireEvent.click(within(localCard!).getByRole('button', { name: 'Import repository' }));

    expect(await within(localCard!).findByText('Imported repository: project-alpha')).toBeInTheDocument();

    const resetButton = within(localCard!).getByRole('button', { name: 'Reset to local root' });
    fireEvent.click(resetButton);

    expect(pathInput.value).toBe('/srv/git/mirror');
    expect(within(localCard!).queryByText('Imported repository: project-alpha')).not.toBeInTheDocument();
    expect(within(localCard!).queryByText('Repository id: repo-77')).not.toBeInTheDocument();
    expect(within(localCard!).queryByRole('link', { name: 'Open repository detail' })).not.toBeInTheDocument();
    expect(within(localCard!).queryByRole('button', { name: 'Reset to local root' })).not.toBeInTheDocument();
  });

  it('disables local connection edit and delete controls while a repository import is in flight', async () => {
    window.location.hash = '#/settings/connections';

    let resolveImport: ((value: Response) => void) | null = null;
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'Local Mirror',
            kind: 'local',
            config: {
              provider: 'local',
              repo_path: '/srv/git/mirror',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/repositories/import/local' && init?.method === 'POST') {
        return await new Promise<Response>((resolve) => {
          resolveImport = resolve;
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const localCard = screen.getByText('Local Mirror').closest('article');
    expect(githubCard).toBeInTheDocument();
    expect(localCard).toBeInTheDocument();

    fireEvent.change(within(localCard!).getByLabelText('Repository path'), {
      target: { value: '/srv/git/mirror/project-alpha' },
    });
    fireEvent.click(within(localCard!).getByRole('button', { name: 'Import repository' }));

    await waitFor(() => {
      expect(within(localCard!).getByRole('button', { name: 'Importing…' })).toBeDisabled();
    });
    expect(within(localCard!).getByRole('button', { name: 'Edit Local Mirror' })).toBeDisabled();
    expect(within(localCard!).getByRole('button', { name: 'Delete Local Mirror' })).toBeDisabled();
    expect(within(githubCard!).getByRole('button', { name: 'Edit GitHub Cloud' })).not.toBeDisabled();
    expect(within(githubCard!).getByRole('button', { name: 'Delete GitHub Cloud' })).not.toBeDisabled();

    resolveImport?.(
      jsonResponse({
        repository: {
          id: 'repo-77',
          name: 'project-alpha',
          default_branch: 'main',
          connection_id: 'conn-2',
          sync_state: 'ready',
        },
        connection: {
          id: 'conn-2',
          name: 'Local Mirror',
          kind: 'local',
        },
      })
    );

    expect(await within(localCard!).findByText('Imported repository: project-alpha')).toBeInTheDocument();
    expect(within(localCard!).getByRole('button', { name: 'Edit Local Mirror' })).not.toBeDisabled();
    expect(within(localCard!).getByRole('button', { name: 'Delete Local Mirror' })).not.toBeDisabled();
  });

  it('disables an open local connection edit form while a repository import is in flight', async () => {
    window.location.hash = '#/settings/connections';

    let resolveImport: ((value: Response) => void) | null = null;
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-2',
            name: 'Local Mirror',
            kind: 'local',
            config: {
              provider: 'local',
              repo_path: '/srv/git/mirror',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/repositories/import/local' && init?.method === 'POST') {
        return await new Promise<Response>((resolve) => {
          resolveImport = resolve;
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const localCard = screen.getByText('Local Mirror').closest('article');
    expect(localCard).toBeInTheDocument();

    fireEvent.click(within(localCard!).getByRole('button', { name: 'Edit Local Mirror' }));
    fireEvent.change(within(localCard!).getByLabelText('Repository path'), {
      target: { value: '/srv/git/mirror/project-alpha' },
    });
    fireEvent.click(within(localCard!).getByRole('button', { name: 'Import repository' }));

    await waitFor(() => {
      expect(within(localCard!).getByRole('button', { name: 'Importing…' })).toBeDisabled();
    });
    expect(screen.getByLabelText('Edit connection name')).toBeDisabled();
    expect(screen.getByLabelText('Edit repo path')).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Save changes' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Cancel' })).toBeDisabled();

    resolveImport?.(
      jsonResponse({
        repository: {
          id: 'repo-77',
          name: 'project-alpha',
          default_branch: 'main',
          connection_id: 'conn-2',
          sync_state: 'ready',
        },
        connection: {
          id: 'conn-2',
          name: 'Local Mirror',
          kind: 'local',
        },
      })
    );

    expect(await within(localCard!).findByText('Imported repository: project-alpha')).toBeInTheDocument();
    expect(screen.getByLabelText('Edit connection name')).not.toBeDisabled();
    expect(screen.getByLabelText('Edit repo path')).not.toBeDisabled();
    expect(screen.getByRole('button', { name: 'Save changes' })).not.toBeDisabled();
    expect(screen.getByRole('button', { name: 'Cancel' })).not.toBeDisabled();
  });

  it('disables a local connection import form while that connection update is in flight', async () => {
    window.location.hash = '#/settings/connections';

    let resolveUpdate: ((value: Response) => void) | null = null;
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-2',
            name: 'Local Mirror',
            kind: 'local',
            config: {
              provider: 'local',
              repo_path: '/srv/git/mirror',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/connections/conn-2' && init?.method === 'PUT') {
        return await new Promise<Response>((resolve) => {
          resolveUpdate = resolve;
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const localCard = screen.getByText('Local Mirror').closest('article');
    expect(localCard).toBeInTheDocument();

    fireEvent.click(within(localCard!).getByRole('button', { name: 'Edit Local Mirror' }));
    fireEvent.change(screen.getByLabelText('Edit repo path'), {
      target: { value: '/srv/git/updated-root' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Saving…' })).toBeDisabled();
    });
    expect(within(localCard!).getByLabelText('Repository path')).toBeDisabled();
    expect(within(localCard!).getByRole('button', { name: 'Import repository' })).toBeDisabled();

    resolveUpdate?.(
      jsonResponse({
        id: 'conn-2',
        name: 'Local Mirror',
        kind: 'local',
        config: {
          provider: 'local',
          repo_path: '/srv/git/updated-root',
        },
      })
    );

    await waitFor(() => {
      expect(within(localCard!).getByLabelText('Repository path')).toHaveValue('/srv/git/updated-root');
    });
    expect(within(localCard!).getByRole('button', { name: 'Import repository' })).not.toBeDisabled();
  });

  it('disables a local connection import form while that connection deletion is in flight', async () => {
    window.location.hash = '#/settings/connections';

    let resolveDelete: ((value: Response) => void) | null = null;
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-2',
            name: 'Local Mirror',
            kind: 'local',
            config: {
              provider: 'local',
              repo_path: '/srv/git/mirror',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/connections/conn-2' && init?.method === 'DELETE') {
        return await new Promise<Response>((resolve) => {
          resolveDelete = resolve;
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const localCard = screen.getByText('Local Mirror').closest('article');
    expect(localCard).toBeInTheDocument();

    fireEvent.click(within(localCard!).getByRole('button', { name: 'Delete Local Mirror' }));

    await waitFor(() => {
      expect(within(localCard!).getByRole('button', { name: 'Deleting…' })).toBeDisabled();
    });
    expect(within(localCard!).getByLabelText('Repository path')).toBeDisabled();
    expect(within(localCard!).getByRole('button', { name: 'Import repository' })).toBeDisabled();

    resolveDelete?.(jsonResponse({}, true, 204));
    await waitFor(() => {
      expect(screen.queryByText('Local Mirror')).not.toBeInTheDocument();
    });
  });

  it('shows a scoped repository import failure on the affected local connection card only', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'Local Mirror',
            kind: 'local',
            config: {
              provider: 'local',
              repo_path: '/srv/git/mirror',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/repositories/import/local' && init?.method === 'POST') {
        return jsonResponse({}, false, 422);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const githubCard = screen.getByText('GitHub Cloud').closest('article');
    const localCard = screen.getByText('Local Mirror').closest('article');

    fireEvent.change(within(localCard!).getByLabelText('Repository path'), {
      target: { value: '/srv/git/mirror/missing-repo' },
    });
    fireEvent.click(within(localCard!).getByRole('button', { name: 'Import repository' }));

    expect(await within(localCard!).findByText('Failed to import repository: Request failed: 422')).toBeInTheDocument();
    expect(within(localCard!).queryByText(/Imported repository:/i)).not.toBeInTheDocument();
    expect(within(githubCard!).queryByText(/Failed to import repository:/i)).not.toBeInTheDocument();
    expect(within(githubCard!).queryByLabelText('Repository path')).not.toBeInTheDocument();
  });

  it('clears stale local import failure details after the repository path changes', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-2',
            name: 'Local Mirror',
            kind: 'local',
            config: {
              provider: 'local',
              repo_path: '/srv/git/mirror',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/repositories/import/local' && init?.method === 'POST') {
        return jsonResponse({}, false, 422);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const localCard = screen.getByText('Local Mirror').closest('article');
    expect(localCard).toBeInTheDocument();

    fireEvent.change(within(localCard!).getByLabelText('Repository path'), {
      target: { value: '/srv/git/mirror/missing-repo' },
    });
    fireEvent.click(within(localCard!).getByRole('button', { name: 'Import repository' }));

    expect(await within(localCard!).findByText('Failed to import repository: Request failed: 422')).toBeInTheDocument();

    fireEvent.change(within(localCard!).getByLabelText('Repository path'), {
      target: { value: '/srv/git/mirror/project-alpha' },
    });

    expect(within(localCard!).queryByText('Failed to import repository: Request failed: 422')).not.toBeInTheDocument();
  });

  it('resets a local connection import form to the updated repo root after editing that connection', async () => {
    window.location.hash = '#/settings/connections';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'Local Mirror',
            kind: 'local',
            config: {
              provider: 'local',
              repo_path: '/srv/git/mirror',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/repositories/import/local' && init?.method === 'POST') {
        expect(init.body).toBe(
          JSON.stringify({
            connection_id: 'conn-1',
            path: '/srv/git/mirror/project-alpha',
          })
        );

        return jsonResponse({
          repository: {
            id: 'repo-77',
            name: 'project-alpha',
            default_branch: 'main',
            connection_id: 'conn-1',
            sync_state: 'ready',
          },
          connection: {
            id: 'conn-1',
            name: 'Local Mirror',
            kind: 'local',
          },
        });
      }

      if (url === '/api/v1/auth/connections/conn-1' && init?.method === 'PUT') {
        expect(init.headers).toEqual({ 'Content-Type': 'application/json' });
        expect(init.body).toBe(
          JSON.stringify({
            name: 'Local Mirror',
            kind: 'local',
            config: {
              provider: 'local',
              repo_path: '/srv/git/updated-root',
            },
          })
        );

        return jsonResponse({
          id: 'conn-1',
          name: 'Local Mirror',
          kind: 'local',
          config: {
            provider: 'local',
            repo_path: '/srv/git/updated-root',
          },
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const localCard = screen.getByText('Local Mirror').closest('article');
    expect(localCard).toBeInTheDocument();

    fireEvent.change(within(localCard!).getByLabelText('Repository path'), {
      target: { value: '/srv/git/mirror/project-alpha' },
    });
    fireEvent.click(within(localCard!).getByRole('button', { name: 'Import repository' }));

    expect(await within(localCard!).findByText('Imported repository: project-alpha')).toBeInTheDocument();
    expect(within(localCard!).getByText('Repository id: repo-77')).toBeInTheDocument();

    fireEvent.click(within(localCard!).getByRole('button', { name: 'Edit Local Mirror' }));
    fireEvent.change(screen.getByLabelText('Edit repo path'), {
      target: { value: '/srv/git/updated-root' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/connections');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/repository-sync-jobs');
      expect(fetchMock).toHaveBeenNthCalledWith(3, '/api/v1/auth/repositories/import/local', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          connection_id: 'conn-1',
          path: '/srv/git/mirror/project-alpha',
        }),
      });
      expect(fetchMock).toHaveBeenNthCalledWith(4, '/api/v1/auth/connections/conn-1', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name: 'Local Mirror',
          kind: 'local',
          config: {
            provider: 'local',
            repo_path: '/srv/git/updated-root',
          },
        }),
      });
    });

    await waitFor(() => {
      expect(within(localCard!).getByLabelText('Repository path')).toHaveValue('/srv/git/updated-root');
    });
    expect(within(localCard!).queryByText(/Imported repository:/i)).not.toBeInTheDocument();
    expect(within(localCard!).queryByText(/Failed to import repository:/i)).not.toBeInTheDocument();
    expect(screen.queryByLabelText('Edit connection name')).not.toBeInTheDocument();
  });

  it('clears stale imported repository success details after editing a local connection', async () => {
    window.location.hash = '#/settings/connections';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'Local Mirror',
            kind: 'local',
            config: {
              provider: 'local',
              repo_path: '/srv/git/mirror',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/repositories/import/local' && init?.method === 'POST') {
        expect(init.body).toBe(
          JSON.stringify({
            connection_id: 'conn-1',
            path: '/srv/git/mirror/project-alpha',
          })
        );

        return jsonResponse({
          repository: {
            id: 'repo-77',
            name: 'project-alpha',
            default_branch: 'main',
            connection_id: 'conn-1',
            sync_state: 'ready',
          },
          connection: {
            id: 'conn-1',
            name: 'Local Mirror',
            kind: 'local',
          },
        });
      }

      if (url === '/api/v1/auth/connections/conn-1' && init?.method === 'PUT') {
        expect(init.headers).toEqual({ 'Content-Type': 'application/json' });
        expect(init.body).toBe(
          JSON.stringify({
            name: 'Local Mirror',
            kind: 'local',
            config: {
              provider: 'local',
              repo_path: '/srv/git/updated-root',
            },
          })
        );

        return jsonResponse({
          id: 'conn-1',
          name: 'Local Mirror',
          kind: 'local',
          config: {
            provider: 'local',
            repo_path: '/srv/git/updated-root',
          },
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const localCard = screen.getByText('Local Mirror').closest('article');
    expect(localCard).toBeInTheDocument();

    fireEvent.change(within(localCard!).getByLabelText('Repository path'), {
      target: { value: '/srv/git/mirror/project-alpha' },
    });
    fireEvent.click(within(localCard!).getByRole('button', { name: 'Import repository' }));

    expect(await within(localCard!).findByText('Imported repository: project-alpha')).toBeInTheDocument();
    expect(within(localCard!).getByText('Repository id: repo-77')).toBeInTheDocument();
    expect(within(localCard!).getByRole('link', { name: 'Open repository detail' })).toHaveAttribute(
      'href',
      '#/repos/repo-77'
    );

    fireEvent.click(within(localCard!).getByRole('button', { name: 'Edit Local Mirror' }));
    fireEvent.change(screen.getByLabelText('Edit repo path'), {
      target: { value: '/srv/git/updated-root' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/connections');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/repository-sync-jobs');
      expect(fetchMock).toHaveBeenNthCalledWith(3, '/api/v1/auth/repositories/import/local', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          connection_id: 'conn-1',
          path: '/srv/git/mirror/project-alpha',
        }),
      });
      expect(fetchMock).toHaveBeenNthCalledWith(4, '/api/v1/auth/connections/conn-1', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name: 'Local Mirror',
          kind: 'local',
          config: {
            provider: 'local',
            repo_path: '/srv/git/updated-root',
          },
        }),
      });
    });

    await waitFor(() => {
      expect(within(localCard!).getByLabelText('Repository path')).toHaveValue('/srv/git/updated-root');
    });
    expect(within(localCard!).queryByText('Imported repository: project-alpha')).not.toBeInTheDocument();
    expect(within(localCard!).queryByText('Repository id: repo-77')).not.toBeInTheDocument();
    expect(within(localCard!).queryByRole('link', { name: 'Open repository detail' })).not.toBeInTheDocument();
    expect(screen.queryByLabelText('Edit connection name')).not.toBeInTheDocument();
  });

  it('clears stale local import failure details after editing a local connection', async () => {
    window.location.hash = '#/settings/connections';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'Local Mirror',
            kind: 'local',
            config: {
              provider: 'local',
              repo_path: '/srv/git/mirror',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/repositories/import/local' && init?.method === 'POST') {
        expect(init.body).toBe(
          JSON.stringify({
            connection_id: 'conn-1',
            path: '/srv/git/mirror/missing-repo',
          })
        );

        return jsonResponse({}, false, 422);
      }

      if (url === '/api/v1/auth/connections/conn-1' && init?.method === 'PUT') {
        expect(init.headers).toEqual({ 'Content-Type': 'application/json' });
        expect(init.body).toBe(
          JSON.stringify({
            name: 'Local Mirror',
            kind: 'local',
            config: {
              provider: 'local',
              repo_path: '/srv/git/updated-root',
            },
          })
        );

        return jsonResponse({
          id: 'conn-1',
          name: 'Local Mirror',
          kind: 'local',
          config: {
            provider: 'local',
            repo_path: '/srv/git/updated-root',
          },
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();

    const localCard = screen.getByText('Local Mirror').closest('article');
    expect(localCard).toBeInTheDocument();

    fireEvent.change(within(localCard!).getByLabelText('Repository path'), {
      target: { value: '/srv/git/mirror/missing-repo' },
    });
    fireEvent.click(within(localCard!).getByRole('button', { name: 'Import repository' }));

    expect(await within(localCard!).findByText('Failed to import repository: Request failed: 422')).toBeInTheDocument();

    fireEvent.click(within(localCard!).getByRole('button', { name: 'Edit Local Mirror' }));
    fireEvent.change(screen.getByLabelText('Edit repo path'), {
      target: { value: '/srv/git/updated-root' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/connections');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/repository-sync-jobs');
      expect(fetchMock).toHaveBeenNthCalledWith(3, '/api/v1/auth/repositories/import/local', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          connection_id: 'conn-1',
          path: '/srv/git/mirror/missing-repo',
        }),
      });
      expect(fetchMock).toHaveBeenNthCalledWith(4, '/api/v1/auth/connections/conn-1', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name: 'Local Mirror',
          kind: 'local',
          config: {
            provider: 'local',
            repo_path: '/srv/git/updated-root',
          },
        }),
      });
    });

    await waitFor(() => {
      expect(within(localCard!).getByLabelText('Repository path')).toHaveValue('/srv/git/updated-root');
    });
    expect(within(localCard!).queryByText(/Imported repository:/i)).not.toBeInTheDocument();
    expect(within(localCard!).queryByText('Failed to import repository: Request failed: 422')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('Edit connection name')).not.toBeInTheDocument();
  });

  it('edits an authenticated github connection from the settings route using the update api', async () => {
    window.location.hash = '#/settings/connections';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'Local Mirror',
            kind: 'local',
            config: {
              provider: 'local',
              repo_path: '/srv/git/mirror',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/connections/conn-1' && init?.method === 'PUT') {
        expect(init.headers).toEqual({ 'Content-Type': 'application/json' });
        expect(init.body).toBe(
          JSON.stringify({
            name: ' GitHub Enterprise ',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.enterprise.example.com',
            },
          })
        );

        return jsonResponse({
          id: 'conn-1',
          name: 'GitHub Enterprise',
          kind: 'github',
          config: {
            provider: 'github',
            base_url: 'https://github.enterprise.example.com',
          },
        });
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    expect(screen.getByText('GitHub Cloud')).toBeInTheDocument();
    expect(screen.getByText('Base URL: https://github.com')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Edit GitHub Cloud' }));
    fireEvent.change(screen.getByLabelText('Edit connection name'), { target: { value: ' GitHub Enterprise ' } });
    fireEvent.change(screen.getByLabelText('Edit base URL'), {
      target: { value: 'https://github.enterprise.example.com' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    expect(await screen.findByText('GitHub Enterprise')).toBeInTheDocument();
    expect(screen.getByText('Base URL: https://github.enterprise.example.com')).toBeInTheDocument();
    expect(screen.getByText('Kind: github')).toBeInTheDocument();
    expect(screen.getByText('Connection id: conn-1')).toBeInTheDocument();
    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/connections');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/repository-sync-jobs');
      expect(fetchMock).toHaveBeenNthCalledWith(3, '/api/v1/auth/connections/conn-1', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name: ' GitHub Enterprise ',
          kind: 'github',
          config: {
            provider: 'github',
            base_url: 'https://github.enterprise.example.com',
          },
        }),
      });
    });
    expect(screen.queryByLabelText('Edit connection name')).not.toBeInTheDocument();
  });

  it('edits an authenticated generic git connection from the settings route using the host-style base_url contract', async () => {
    window.location.hash = '#/settings/connections';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'Generic Git Mirror',
            kind: 'generic_git',
            config: {
              provider: 'generic_git',
              base_url: 'https://git.example.com',
            },
          },
          {
            id: 'conn-2',
            name: 'Local Mirror',
            kind: 'local',
            config: {
              provider: 'local',
              repo_path: '/srv/git/mirror',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/connections/conn-1' && init?.method === 'PUT') {
        expect(init.headers).toEqual({ 'Content-Type': 'application/json' });
        expect(init.body).toBe(
          JSON.stringify({
            name: ' Generic Git Host ',
            kind: 'generic_git',
            config: {
              provider: 'generic_git',
              base_url: 'https://git.internal.example.com',
            },
          })
        );

        return jsonResponse({
          id: 'conn-1',
          name: 'Generic Git Host',
          kind: 'generic_git',
          config: {
            provider: 'generic_git',
            base_url: 'https://git.internal.example.com',
          },
        });
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    expect(screen.getByText('Generic Git Mirror')).toBeInTheDocument();
    expect(screen.getByText('Base URL: https://git.example.com')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Edit Generic Git Mirror' }));
    fireEvent.change(screen.getByLabelText('Edit connection name'), { target: { value: ' Generic Git Host ' } });
    fireEvent.change(screen.getByLabelText('Edit base URL'), {
      target: { value: 'https://git.internal.example.com' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    expect(await screen.findByText('Generic Git Host')).toBeInTheDocument();
    expect(screen.getByText('Base URL: https://git.internal.example.com')).toBeInTheDocument();
    expect(screen.getByText('Kind: generic_git')).toBeInTheDocument();
    expect(screen.getByText('Connection id: conn-1')).toBeInTheDocument();
    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/connections');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/repository-sync-jobs');
      expect(fetchMock).toHaveBeenNthCalledWith(3, '/api/v1/auth/connections/conn-1', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name: ' Generic Git Host ',
          kind: 'generic_git',
          config: {
            provider: 'generic_git',
            base_url: 'https://git.internal.example.com',
          },
        }),
      });
    });
    expect(screen.queryByLabelText('Edit connection name')).not.toBeInTheDocument();
  });

  it('shows scoped generic git connection edit failures on the settings route while keeping the host-style base_url contract', async () => {
    window.location.hash = '#/settings/connections';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'Generic Git Mirror',
            kind: 'generic_git',
            config: {
              provider: 'generic_git',
              base_url: 'https://git.example.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/connections/conn-1' && init?.method === 'PUT') {
        expect(init.headers).toEqual({ 'Content-Type': 'application/json' });
        expect(init.body).toBe(
          JSON.stringify({
            name: ' Scoped generic failure ',
            kind: 'generic_git',
            config: {
              provider: 'generic_git',
              base_url: 'https://git.internal.example.com',
            },
          })
        );

        return jsonResponse({}, false, 403);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    const connectionCard = screen.getByText('Generic Git Mirror').closest('article');
    expect(connectionCard).not.toBeNull();
    expect(screen.getByText('Base URL: https://git.example.com')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Edit Generic Git Mirror' }));
    fireEvent.change(screen.getByLabelText('Edit connection name'), { target: { value: ' Scoped generic failure ' } });
    fireEvent.change(screen.getByLabelText('Edit base URL'), {
      target: { value: 'https://git.internal.example.com' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    expect(await within(connectionCard as HTMLElement).findByText('Failed to update connection: Request failed: 403')).toBeInTheDocument();
    expect(screen.getByText('Generic Git Mirror')).toBeInTheDocument();
    expect(screen.getByText('Base URL: https://git.example.com')).toBeInTheDocument();
    expect(screen.queryByText('Scoped generic failure')).not.toBeInTheDocument();
    expect(screen.getByLabelText('Edit connection name')).toHaveValue(' Scoped generic failure ');
    expect(screen.getByLabelText('Edit base URL')).toHaveValue('https://git.internal.example.com');
    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/connections');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/repository-sync-jobs');
      expect(fetchMock).toHaveBeenNthCalledWith(3, '/api/v1/auth/connections/conn-1', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name: ' Scoped generic failure ',
          kind: 'generic_git',
          config: {
            provider: 'generic_git',
            base_url: 'https://git.internal.example.com',
          },
        }),
      });
    });
  });

  it('shows scoped generic git connection delete failures on the settings route while keeping the host-style inventory visible', async () => {
    window.location.hash = '#/settings/connections';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'Generic Git Mirror',
            kind: 'generic_git',
            config: {
              provider: 'generic_git',
              base_url: 'https://git.example.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/connections/conn-1' && init?.method === 'DELETE') {
        return jsonResponse({}, false, 403);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    const genericConnectionCard = screen.getByText('Generic Git Mirror').closest('article');
    expect(genericConnectionCard).not.toBeNull();
    expect(screen.getByText('Base URL: https://git.example.com')).toBeInTheDocument();
    expect(screen.getByText('GitHub Cloud')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Delete Generic Git Mirror' }));

    expect(
      await within(genericConnectionCard as HTMLElement).findByText('Failed to delete connection: Request failed: 403')
    ).toBeInTheDocument();
    expect(screen.getByText('Generic Git Mirror')).toBeInTheDocument();
    expect(screen.getByText('Base URL: https://git.example.com')).toBeInTheDocument();
    expect(screen.getByText('GitHub Cloud')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Deleting…' })).not.toBeInTheDocument();
    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/connections');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/repository-sync-jobs');
      expect(fetchMock).toHaveBeenNthCalledWith(3, '/api/v1/auth/connections/conn-1', {
        method: 'DELETE',
      });
    });
  });

  it('removes only the deleted generic git connection from the settings route while keeping the remaining host-style inventory visible', async () => {
    window.location.hash = '#/settings/connections';

    let resolveDelete: ((value: Response) => void) | null = null;
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'Generic Git Mirror',
            kind: 'generic_git',
            config: {
              provider: 'generic_git',
              base_url: 'https://git.example.com',
            },
          },
          {
            id: 'conn-2',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/connections/conn-1' && init?.method === 'DELETE') {
        return await new Promise<Response>((resolve) => {
          resolveDelete = resolve;
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    expect(screen.getByText('Generic Git Mirror')).toBeInTheDocument();
    expect(screen.getByText('Base URL: https://git.example.com')).toBeInTheDocument();
    expect(screen.getByText('GitHub Cloud')).toBeInTheDocument();
    expect(screen.getByText('Base URL: https://github.com')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Delete Generic Git Mirror' }));

    expect(screen.getByRole('button', { name: 'Deleting…' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Delete GitHub Cloud' })).toBeDisabled();

    resolveDelete?.(jsonResponse({}, true, 204));

    await waitFor(() => {
      expect(screen.queryByText('Generic Git Mirror')).not.toBeInTheDocument();
    });
    expect(screen.queryByText('Base URL: https://git.example.com')).not.toBeInTheDocument();
    expect(screen.getByText('GitHub Cloud')).toBeInTheDocument();
    expect(screen.getByText('Base URL: https://github.com')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Delete GitHub Cloud' })).not.toBeDisabled();
    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/connections');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/repository-sync-jobs');
      expect(fetchMock).toHaveBeenNthCalledWith(3, '/api/v1/auth/connections/conn-1', {
        method: 'DELETE',
      });
    });
  });

  it('disables all authenticated connection delete controls while a deletion is in flight and removes the deleted connection', async () => {
    window.location.hash = '#/settings/connections';

    let resolveDelete: ((value: Response) => void) | null = null;
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([
          {
            id: 'conn-1',
            name: 'GitHub Cloud',
            kind: 'github',
            config: {
              provider: 'github',
              base_url: 'https://github.com',
            },
          },
          {
            id: 'conn-2',
            name: 'Local Mirror',
            kind: 'local',
            config: {
              provider: 'local',
              repo_path: '/srv/git/mirror',
            },
          },
        ]);
      }

      if (url === '/api/v1/auth/connections/conn-2' && init?.method === 'DELETE') {
        return await new Promise<Response>((resolve) => {
          resolveDelete = resolve;
        });
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    expect(screen.getByText('GitHub Cloud')).toBeInTheDocument();
    expect(screen.getByText('Local Mirror')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Delete Local Mirror' }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/connections');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/repository-sync-jobs');
      expect(fetchMock).toHaveBeenNthCalledWith(3, '/api/v1/auth/connections/conn-2', {
        method: 'DELETE',
      });
    });
    expect(screen.getByRole('button', { name: 'Deleting…' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Delete GitHub Cloud' })).toBeDisabled();

    resolveDelete?.(jsonResponse({}, true, 204));

    await waitFor(() => {
      expect(screen.queryByText('Local Mirror')).not.toBeInTheDocument();
    });
    expect(screen.getByText('GitHub Cloud')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Delete GitHub Cloud' })).not.toBeDisabled();
  });

  it('renders all supported authenticated connection kind options on the settings route', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([]);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    const kindSelect = screen.getByLabelText('Connection kind');
    const optionValues = Array.from(kindSelect.querySelectorAll('option')).map((option) => option.textContent);
    expect(optionValues).toEqual([
      'github',
      'gitlab',
      'gitea',
      'gerrit',
      'bitbucket',
      'azure_devops',
      'generic_git',
      'local',
    ]);
  });

  it('shows authenticated connection management controls only after the initial settings inventory finishes loading', async () => {
    window.location.hash = '#/settings/connections';

    let resolveConnections: ((value: Response) => void) | null = null;
    vi.spyOn(globalThis, 'fetch').mockImplementation(
      (async (input, init) => {
        const url = String(input);

        if (url === '/api/v1/auth/connections' && !init) {
          return await new Promise<Response>((resolve) => {
            resolveConnections = resolve;
          });
        }

        throw new Error(`Unhandled fetch: ${url}`);
      }) as typeof fetch
    );

    render(<App />);

    expect(await screen.findByText('Loading connections…')).toBeInTheDocument();
    expect(screen.queryByLabelText('Connection name')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('Connection kind')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Create connection' })).not.toBeInTheDocument();

    resolveConnections?.(jsonResponse([]));

    await waitFor(() => {
      expect(screen.queryByText('Loading connections…')).not.toBeInTheDocument();
    });
    expect(screen.getByLabelText('Connection name')).toBeInTheDocument();
    expect(screen.getByLabelText('Connection kind')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Create connection' })).not.toBeDisabled();
  });

  it('shows scoped generic git connection creation failures on the settings route while keeping the host-style base_url contract', async () => {
    window.location.hash = '#/settings/connections';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/repository-sync-jobs' && !init) {
        return jsonResponse([]);
      }

      if (url === '/api/v1/auth/connections' && init?.method === 'POST') {
        expect(init.headers).toEqual({ 'Content-Type': 'application/json' });
        expect(init.body).toBe(
          JSON.stringify({
            name: 'Scoped generic failure',
            kind: 'generic_git',
            config: {
              provider: 'generic_git',
              base_url: 'https://git.internal.example.com',
            },
          })
        );

        return jsonResponse({}, false, 403);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText('Connection name'), { target: { value: 'Scoped generic failure' } });
    fireEvent.change(screen.getByLabelText('Connection kind'), { target: { value: 'generic_git' } });
    fireEvent.change(screen.getByLabelText('Base URL'), { target: { value: 'https://git.internal.example.com' } });
    fireEvent.click(screen.getByRole('button', { name: 'Create connection' }));

    expect(await screen.findByText('Failed to create connection: Request failed: 403')).toBeInTheDocument();
    expect(screen.queryByText('Scoped generic failure')).not.toBeInTheDocument();
    await waitFor(() => {
      expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/v1/auth/connections');
      expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/v1/auth/repository-sync-jobs');
      expect(fetchMock).toHaveBeenNthCalledWith(3, '/api/v1/auth/connections', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name: 'Scoped generic failure',
          kind: 'generic_git',
          config: {
            provider: 'generic_git',
            base_url: 'https://git.internal.example.com',
          },
        }),
      });
    });
  });

  it('hides authenticated connection management controls when the settings inventory cannot be loaded', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections') {
        return jsonResponse({}, false, 403);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Failed to load connections: Request failed: 403')).toBeInTheDocument();
    expect(screen.queryByLabelText('Connection name')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('Connection kind')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Create connection' })).not.toBeInTheDocument();
  });

  it('shows authenticated connection loading failures on the settings route', async () => {
    window.location.hash = '#/settings/connections';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input);

      if (url === '/api/v1/auth/connections') {
        return jsonResponse({}, false, 403);
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Authenticated connections')).toBeInTheDocument();
    expect(await screen.findByText('Failed to load connections: Request failed: 403')).toBeInTheDocument();
  });

  it('finds definitions for the selected file symbol and renders ordered navigable candidates with revision metadata', async () => {
    window.location.hash = '#/repos/repo-42';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input);

      if (url === '/api/v1/repos/repo-42') {
        return jsonResponse({
          repository: {
            id: 'repo-42',
            name: 'beta-repo',
            default_branch: 'develop',
            connection_id: 'conn-7',
            sync_state: 'ready',
          },
          connection: {
            id: 'conn-7',
            name: 'GitHub App',
            kind: 'github',
          },
        });
      }

      if (url === '/api/v1/repos/repo-42/tree?path=') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: '',
          entries: [{ name: 'src', path: 'src', kind: 'dir' }],
        });
      }

      if (url === '/api/v1/repos/repo-42/tree?path=src') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'src',
          entries: [
            { name: 'main.rs', path: 'src/main.rs', kind: 'file' },
            { name: 'lib.rs', path: 'src/lib.rs', kind: 'file' },
          ],
        });
      }

      if (url === '/api/v1/repos/repo-42/blob?path=src%2Fmain.rs') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'src/main.rs',
          size_bytes: 28,
          content: 'fn helper() { lib::helper(); }',
        });
      }

      if (url === '/api/v1/repos/repo-42/definitions?path=src%2Fmain.rs&symbol=helper') {
        return jsonResponse({
          status: 'supported',
          repo_id: 'repo-42',
          path: 'src/main.rs',
          revision: 'rev-def-123',
          symbol: 'helper',
          definitions: [
            {
              path: 'src/lib.rs',
              name: 'helper',
              kind: 'function',
              range: { start_line: 3, end_line: 6 },
              browse_url: '/api/v1/repos/repo-42/blob?path=src%2Flib.rs&revision=rev-def-123#L3',
            },
            {
              path: 'src/main.rs',
              name: 'helper',
              kind: 'function',
              range: { start_line: 10, end_line: 12 },
              browse_url: '/api/v1/repos/repo-42/blob?path=src%2Fmain.rs&revision=rev-def-123#L10',
            },
          ],
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('beta-repo')).toBeInTheDocument();
    fireEvent.click((await screen.findByText('src/')).closest('button')!);
    fireEvent.click((await screen.findByText('main.rs')).closest('button')!);

    expect(await screen.findByText('fn helper() { lib::helper(); }')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Symbol token'), { target: { value: 'helper' } });
    fireEvent.click(screen.getByRole('button', { name: 'Find definitions' }));

    expect(await screen.findByText('Definition results')).toBeInTheDocument();
    expect(screen.getByText('Revision: rev-def-123')).toBeInTheDocument();
    expect(screen.getAllByText('helper').length).toBeGreaterThan(0);
    expect(screen.getByText('src/lib.rs')).toBeInTheDocument();
    expect(screen.getByText('Lines 3–6')).toBeInTheDocument();
    expect(screen.getAllByText('src/main.rs').length).toBeGreaterThan(0);
    expect(screen.getByText('Lines 10–12')).toBeInTheDocument();
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/repos/repo-42/definitions?path=src%2Fmain.rs&symbol=helper');
  });

  it('ignores stale navigation responses after the user switches files', async () => {
    window.location.hash = '#/repos/repo-42';

    const pendingDefinitions = deferredResponse();
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input);

      if (url === '/api/v1/repos/repo-42') {
        return jsonResponse({
          repository: {
            id: 'repo-42',
            name: 'beta-repo',
            default_branch: 'develop',
            connection_id: 'conn-7',
            sync_state: 'ready',
          },
          connection: {
            id: 'conn-7',
            name: 'GitHub App',
            kind: 'github',
          },
        });
      }

      if (url === '/api/v1/repos/repo-42/tree?path=') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: '',
          entries: [{ name: 'src', path: 'src', kind: 'dir' }],
        });
      }

      if (url === '/api/v1/repos/repo-42/tree?path=src') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'src',
          entries: [
            { name: 'main.rs', path: 'src/main.rs', kind: 'file' },
            { name: 'lib.rs', path: 'src/lib.rs', kind: 'file' },
          ],
        });
      }

      if (url === '/api/v1/repos/repo-42/blob?path=src%2Fmain.rs') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'src/main.rs',
          size_bytes: 20,
          content: 'fn helper() {}',
        });
      }

      if (url === '/api/v1/repos/repo-42/blob?path=src%2Flib.rs') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'src/lib.rs',
          size_bytes: 24,
          content: 'export const value = 42;',
        });
      }

      if (url === '/api/v1/repos/repo-42/definitions?path=src%2Fmain.rs&symbol=helper') {
        return pendingDefinitions.promise;
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('beta-repo')).toBeInTheDocument();
    fireEvent.click((await screen.findByText('src/')).closest('button')!);
    fireEvent.click((await screen.findByText('main.rs')).closest('button')!);
    expect(await screen.findByText('fn helper() {}')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Symbol token'), { target: { value: 'helper' } });
    fireEvent.click(screen.getByRole('button', { name: 'Find definitions' }));
    expect(await screen.findByText('Loading code navigation…')).toBeInTheDocument();

    fireEvent.click((await screen.findByText('lib.rs')).closest('button')!);
    expect(await screen.findByText('export const value = 42;')).toBeInTheDocument();
    expect(screen.queryByText('Definition results')).not.toBeInTheDocument();

    pendingDefinitions.resolve(
      jsonResponse({
        status: 'supported',
        repo_id: 'repo-42',
        path: 'src/main.rs',
        revision: 'rev-stale-123',
        symbol: 'helper',
        definitions: [
          {
            path: 'src/main.rs',
            name: 'helper',
            kind: 'function',
            range: { start_line: 1, end_line: 1 },
            browse_url: '/api/v1/repos/repo-42/blob?path=src%2Fmain.rs&revision=rev-stale-123#L1',
          },
        ],
      })
    );

    await waitFor(() => {
      expect(screen.queryByText('Loading code navigation…')).not.toBeInTheDocument();
      expect(screen.queryByText('Definition results')).not.toBeInTheDocument();
      expect(screen.getByText('export const value = 42;')).toBeInTheDocument();
    });
  });

  it('finds references and opens the referenced file when a result is clicked', async () => {
    window.location.hash = '#/repos/repo-42';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input);

      if (url === '/api/v1/repos/repo-42') {
        return jsonResponse({
          repository: {
            id: 'repo-42',
            name: 'beta-repo',
            default_branch: 'develop',
            connection_id: 'conn-7',
            sync_state: 'ready',
          },
          connection: {
            id: 'conn-7',
            name: 'GitHub App',
            kind: 'github',
          },
        });
      }

      if (url === '/api/v1/repos/repo-42/tree?path=') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: '',
          entries: [{ name: 'src', path: 'src', kind: 'dir' }],
        });
      }

      if (url === '/api/v1/repos/repo-42/tree?path=src') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'src',
          entries: [
            { name: 'lib.rs', path: 'src/lib.rs', kind: 'file' },
            { name: 'consumer.rs', path: 'src/consumer.rs', kind: 'file' },
          ],
        });
      }

      if (url === '/api/v1/repos/repo-42/blob?path=src%2Flib.rs') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'src/lib.rs',
          size_bytes: 24,
          content: 'pub fn helper() {}',
        });
      }

      if (url === '/api/v1/repos/repo-42/blob?path=src%2Fconsumer.rs&revision=rev-ref-456') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'src/consumer.rs',
          size_bytes: 37,
          content: 'fn run() { crate::helper(); helper(); }',
        });
      }

      if (url === '/api/v1/repos/repo-42/references?path=src%2Flib.rs&symbol=helper') {
        return jsonResponse({
          status: 'supported',
          repo_id: 'repo-42',
          path: 'src/lib.rs',
          revision: 'rev-ref-456',
          symbol: 'helper',
          references: [
            {
              path: 'src/consumer.rs',
              line_number: 8,
              line: 'fn run() { crate::helper(); helper(); }',
              browse_url: '/api/v1/repos/repo-42/blob?path=src%2Fconsumer.rs&revision=rev-ref-456#L8',
            },
          ],
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('beta-repo')).toBeInTheDocument();
    fireEvent.click((await screen.findByText('src/')).closest('button')!);
    fireEvent.click((await screen.findByText('lib.rs')).closest('button')!);

    expect(await screen.findByText('pub fn helper() {}')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Symbol token'), { target: { value: 'helper' } });
    fireEvent.click(screen.getByRole('button', { name: 'Find references' }));

    expect(await screen.findByText('Reference results')).toBeInTheDocument();
    expect(screen.getByText('Revision: rev-ref-456')).toBeInTheDocument();
    expect(screen.getByText('Line 8')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /src\/consumer\.rs/i }));

    expect(await screen.findByText('src/consumer.rs')).toBeInTheDocument();
    expect(screen.getAllByText('fn run() { crate::helper(); helper(); }').length).toBeGreaterThan(0);
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/repos/repo-42/references?path=src%2Flib.rs&symbol=helper');
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/repos/repo-42/blob?path=src%2Fconsumer.rs&revision=rev-ref-456');
  });

  it('shows a non-fatal capability notice for unsupported navigation responses', async () => {
    window.location.hash = '#/repos/repo-42';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input);

      if (url === '/api/v1/repos/repo-42') {
        return jsonResponse({
          repository: {
            id: 'repo-42',
            name: 'beta-repo',
            default_branch: 'develop',
            connection_id: 'conn-7',
            sync_state: 'ready',
          },
          connection: {
            id: 'conn-7',
            name: 'GitHub App',
            kind: 'github',
          },
        });
      }

      if (url === '/api/v1/repos/repo-42/tree?path=') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: '',
          entries: [{ name: 'notes.txt', path: 'notes.txt', kind: 'file' }],
        });
      }

      if (url === '/api/v1/repos/repo-42/blob?path=notes.txt') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'notes.txt',
          size_bytes: 18,
          content: 'plain text helper',
        });
      }

      if (url === '/api/v1/repos/repo-42/definitions?path=notes.txt&symbol=helper') {
        return jsonResponse({
          status: 'unsupported',
          repo_id: 'repo-42',
          path: 'notes.txt',
          revision: null,
          symbol: 'helper',
          capability: 'Definitions are not available for plain text files.',
          definitions: [],
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('beta-repo')).toBeInTheDocument();
    fireEvent.click((await screen.findByText('notes.txt')).closest('button')!);
    expect(await screen.findByText('plain text helper')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Symbol token'), { target: { value: 'helper' } });
    fireEvent.click(screen.getByRole('button', { name: 'Find definitions' }));

    expect(await screen.findByText('Definitions are not available for plain text files.')).toBeInTheDocument();
    expect(screen.getByText('plain text helper')).toBeInTheDocument();
  });

  it('shows a friendly no-results state when navigation returns an empty supported list', async () => {
    window.location.hash = '#/repos/repo-42';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input);

      if (url === '/api/v1/repos/repo-42') {
        return jsonResponse({
          repository: {
            id: 'repo-42',
            name: 'beta-repo',
            default_branch: 'develop',
            connection_id: 'conn-7',
            sync_state: 'ready',
          },
          connection: {
            id: 'conn-7',
            name: 'GitHub App',
            kind: 'github',
          },
        });
      }

      if (url === '/api/v1/repos/repo-42/tree?path=') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: '',
          entries: [{ name: 'src', path: 'src', kind: 'dir' }],
        });
      }

      if (url === '/api/v1/repos/repo-42/tree?path=src') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'src',
          entries: [{ name: 'main.rs', path: 'src/main.rs', kind: 'file' }],
        });
      }

      if (url === '/api/v1/repos/repo-42/blob?path=src%2Fmain.rs') {
        return jsonResponse({
          repo_id: 'repo-42',
          path: 'src/main.rs',
          size_bytes: 16,
          content: 'fn helper() {}',
        });
      }

      if (url === '/api/v1/repos/repo-42/references?path=src%2Fmain.rs&symbol=missing') {
        return jsonResponse({
          status: 'supported',
          repo_id: 'repo-42',
          path: 'src/main.rs',
          revision: 'rev-empty-789',
          symbol: 'missing',
          references: [],
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('beta-repo')).toBeInTheDocument();
    fireEvent.click((await screen.findByText('src/')).closest('button')!);
    fireEvent.click((await screen.findByText('main.rs')).closest('button')!);
    expect(await screen.findByText('fn helper() {}')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Symbol token'), { target: { value: 'missing' } });
    fireEvent.click(screen.getByRole('button', { name: 'Find references' }));

    expect(await screen.findByText('No references found for “missing”.')).toBeInTheDocument();
    expect(screen.getByText('Revision: rev-empty-789')).toBeInTheDocument();
  });

  it('searches code and filters results by repository from the dedicated search page', async () => {
    window.location.hash = '#/search';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input);

      if (url === '/api/v1/repos') {
        return jsonResponse([
          {
            id: 'repo-1',
            name: 'alpha-repo',
            default_branch: 'main',
            sync_state: 'ready',
          },
          {
            id: 'repo-2',
            name: 'beta-repo',
            default_branch: 'develop',
            sync_state: 'pending',
          },
        ]);
      }

      if (url === '/api/v1/search?q=needle&repo_id=repo-2') {
        return jsonResponse({
          query: 'needle',
          repo_id: 'repo-2',
          results: [
            {
              repo_id: 'repo-2',
              path: 'src/search.ts',
              line_number: 12,
              line: 'const needle = true;',
            },
          ],
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(screen.getByText('Run API-backed code search across repositories from a dedicated route.')).toBeInTheDocument();
    expect(await screen.findByText('Enter a query to search indexed code.')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Search query'), { target: { value: 'needle' } });
    fireEvent.change(screen.getByLabelText('Repository filter'), { target: { value: 'repo-2' } });
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));

    expect(await screen.findByText('src/search.ts')).toBeInTheDocument();
    expect(screen.getAllByText('beta-repo').length).toBeGreaterThan(0);
    expect(screen.getByText('Line 12')).toBeInTheDocument();
    expect(screen.getByText('const needle = true;')).toBeInTheDocument();
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/search?q=needle&repo_id=repo-2');
  });

  it('opens repository source from dedicated search results and keeps a contextual back link to search', async () => {
    window.location.hash = '#/search';

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input);

      if (url === '/api/v1/repos') {
        return jsonResponse([
          {
            id: 'repo-2',
            name: 'beta-repo',
            default_branch: 'develop',
            sync_state: 'pending',
          },
        ]);
      }

      if (url === '/api/v1/search?q=needle&repo_id=') {
        return jsonResponse({
          query: 'needle',
          repo_id: null,
          results: [
            {
              repo_id: 'repo-2',
              path: 'src/search.ts',
              line_number: 12,
              line: 'const needle = true;',
            },
          ],
        });
      }

      if (url === '/api/v1/repos/repo-2') {
        return jsonResponse({
          repository: {
            id: 'repo-2',
            name: 'beta-repo',
            default_branch: 'develop',
            connection_id: 'conn-7',
            sync_state: 'ready',
          },
          connection: {
            id: 'conn-7',
            name: 'GitHub App',
            kind: 'github',
          },
        });
      }

      if (url === '/api/v1/repos/repo-2/commits?limit=20') {
        return jsonResponse({
          repo_id: 'repo-2',
          commits: [],
        });
      }

      if (url === '/api/v1/repos/repo-2/tree?path=src') {
        return jsonResponse({
          repo_id: 'repo-2',
          path: 'src',
          entries: [{ name: 'search.ts', path: 'src/search.ts', kind: 'file' }],
        });
      }

      if (url === '/api/v1/repos/repo-2/blob?path=src%2Fsearch.ts') {
        return jsonResponse({
          repo_id: 'repo-2',
          path: 'src/search.ts',
          size_bytes: 21,
          content: 'const needle = true;',
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Enter a query to search indexed code.')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Search query'), { target: { value: 'needle' } });
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));

    await waitFor(() => {
      expect(window.location.hash).toBe('#/search?q=needle&repo_id=');
    });

    const openSourceLink = await screen.findByRole('link', { name: 'Open source in repository detail' });
    expect(openSourceLink).toHaveAttribute('href', '#/repos/repo-2?path=src%2Fsearch.ts&from=search&q=needle&repo_id=');

    fireEvent.click(openSourceLink);

    expect(await screen.findByText('Current path: src')).toBeInTheDocument();
    expect(await screen.findByText('src/search.ts')).toBeInTheDocument();
    expect(screen.getByText('const needle = true;')).toBeInTheDocument();

    const backToSearchLink = screen.getByRole('link', { name: '← Back to search results' });
    expect(backToSearchLink).toHaveAttribute('href', '#/search?q=needle&repo_id=');

    fireEvent.click(backToSearchLink);

    await waitFor(() => {
      expect(window.location.hash).toBe('#/search?q=needle&repo_id=');
    });
    expect(await screen.findByLabelText('Search query')).toHaveValue('needle');
    expect(screen.getByText('Results')).toBeInTheDocument();
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/search?q=needle&repo_id=');
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/repos/repo-2/tree?path=src');
    expect(fetchMock).toHaveBeenCalledWith('/api/v1/repos/repo-2/blob?path=src%2Fsearch.ts');
  });

  it('keeps dedicated search deep links pinned to the last submitted search context even if the form is edited afterwards', async () => {
    window.location.hash = '#/search';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input);

      if (url === '/api/v1/repos') {
        return jsonResponse([
          {
            id: 'repo-1',
            name: 'alpha-repo',
            default_branch: 'main',
            sync_state: 'ready',
          },
          {
            id: 'repo-2',
            name: 'beta-repo',
            default_branch: 'develop',
            sync_state: 'pending',
          },
        ]);
      }

      if (url === '/api/v1/search?q=needle&repo_id=repo-2') {
        return jsonResponse({
          query: 'needle',
          repo_id: 'repo-2',
          results: [
            {
              repo_id: 'repo-2',
              path: 'src/search.ts',
              line_number: 12,
              line: 'const needle = true;',
            },
          ],
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Enter a query to search indexed code.')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Search query'), { target: { value: 'needle' } });
    fireEvent.change(screen.getByLabelText('Repository filter'), { target: { value: 'repo-2' } });
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));

    await waitFor(() => {
      expect(window.location.hash).toBe('#/search?q=needle&repo_id=repo-2');
    });

    const openSourceLink = await screen.findByRole('link', { name: 'Open source in repository detail' });
    expect(openSourceLink).toHaveAttribute('href', '#/repos/repo-2?path=src%2Fsearch.ts&from=search&q=needle&repo_id=repo-2');

    fireEvent.change(screen.getByLabelText('Search query'), { target: { value: 'drifted' } });
    fireEvent.change(screen.getByLabelText('Repository filter'), { target: { value: 'repo-1' } });

    expect(screen.getByRole('link', { name: 'Open source in repository detail' })).toHaveAttribute(
      'href',
      '#/repos/repo-2?path=src%2Fsearch.ts&from=search&q=needle&repo_id=repo-2',
    );
  });

  it('shows an empty search state when no matches are returned on the dedicated search page', async () => {
    window.location.hash = '#/search';

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = String(input);

      if (url === '/api/v1/repos') {
        return jsonResponse([
          {
            id: 'repo-1',
            name: 'alpha-repo',
            default_branch: 'main',
            sync_state: 'ready',
          },
        ]);
      }

      if (url === '/api/v1/search?q=missing&repo_id=') {
        return jsonResponse({
          query: 'missing',
          repo_id: null,
          results: [],
        });
      }

      throw new Error(`Unhandled fetch: ${url}`);
    });

    render(<App />);

    expect(await screen.findByText('Enter a query to search indexed code.')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Search query'), { target: { value: 'missing' } });
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));

    expect(await screen.findByText('No matches found for “missing”.')).toBeInTheDocument();
  });

  it('shows an error state when the repository list request fails', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(jsonResponse({}, false, 500));

    render(<App />);

    expect(await screen.findByText('Failed to load: Request failed: 500')).toBeInTheDocument();
  });
});
