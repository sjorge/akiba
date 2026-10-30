/*
 * Serial request queue with a minimum spacing between request starts
 */
import { AnidbError } from "lib/anidb/error";
import { RESPONSE_CODE } from "lib/anidb/constants";

type Job = {
  priority: number;
  order: number;
  run: () => Promise<unknown>;
  resolve: (value: unknown) => void;
  reject: (reason: unknown) => void;
};

/*
 * Runs jobs one at a time with a minimum time between the starts of two jobs,
 * which is how the flood protection of the spec is kept. The lowest priority
 * number runs first, equal priorities run in the order they were scheduled.
 */
export class RateLimiter {
  private queue: Job[] = [];
  private counter: number = 0;
  private running: boolean = false;
  private stopped: boolean = false;
  private lastStart: number = 0;
  private timer: ReturnType<typeof setTimeout> | undefined;

  public constructor(private minInterval: number) {}

  public toString(): string {
    return "RateLimiter";
  }

  /*
   * lower priority numbers run first, equal priorities run in call order
   */
  public schedule<T>(priority: number, run: () => Promise<T>): Promise<T> {
    if (this.stopped) {
      return Promise.reject(new AnidbError(RESPONSE_CODE.NOT_CONNECTED));
    }
    return new Promise<T>((resolve, reject) => {
      this.queue.push({
        priority,
        order: this.counter++,
        run,
        resolve: resolve as (value: unknown) => void,
        reject,
      });
      this.queue.sort((a, b) => a.priority - b.priority || a.order - b.order);
      this.next();
    });
  }

  /*
   * reject all waiting jobs and refuse new ones
   */
  public stop(): void {
    this.stopped = true;
    clearTimeout(this.timer);
    this.timer = undefined;
    for (const job of this.queue.splice(0)) {
      job.reject(new AnidbError(RESPONSE_CODE.NOT_CONNECTED));
    }
  }

  private next(): void {
    if (this.running || this.timer !== undefined) return;
    const job = this.queue.shift();
    if (job === undefined) return;

    const wait = this.lastStart + this.minInterval - Date.now();
    if (wait > 0) {
      this.queue.unshift(job);
      this.timer = setTimeout(() => {
        this.timer = undefined;
        this.next();
      }, wait);
      return;
    }

    this.running = true;
    this.lastStart = Date.now();
    job
      .run()
      .then(job.resolve, job.reject)
      .finally(() => {
        this.running = false;
        this.next();
      });
  }
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
