// Reusable Tauri IPC mock harness for component/hook tests (#115).
//
// Tauri's `invoke(cmd, args)` normally round-trips to the Rust backend. In
// jsdom there's no backend, so `mockCommands` intercepts `invoke` and answers
// from an in-memory map keyed by command name. Any command a test didn't
// register throws — so an unexpected IPC call fails loudly instead of hanging.
//
// Usage:
//   mockCommands({
//     get_settings: () => ({ active_provider: 'anthropic' }),
//     send_chat_message: (args) => `echo: ${args.message}`,
//   });
//
// clearMocks() runs automatically after each test (see setup.ts).
import { mockIPC } from '@tauri-apps/api/mocks';

export type IpcHandler = (args: Record<string, unknown>) => unknown;
export type IpcHandlers = Record<string, IpcHandler>;

export function mockCommands(handlers: IpcHandlers): void {
  mockIPC((cmd, args) => {
    const handler = handlers[cmd];
    if (!handler) {
      throw new Error(`Unmocked Tauri command: "${cmd}"`);
    }
    return handler((args ?? {}) as Record<string, unknown>);
  });
}
