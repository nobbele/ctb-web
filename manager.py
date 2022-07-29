#!/usr/bin/python
import curses
from multiprocessing import Manager
from queue import Queue
import subprocess
import threading


class TerminalWindow:
    def __init__(self, parent_scr: curses.window, height: int, width: int, y: int, x: int, color: int, label: str) -> None:
        self.width = width
        self.height = height
        self.color = color
        self.label = label
        self.is_first_line = True

        self.win = parent_scr.subwin(height, width, y, x)
        self.win.border()

        self.contents = self.win.subpad(height - 2, width - 2, y + 1, x + 1)
        self.contents.scrollok(True)

    def print(self, str: str):
        str = str.strip()
        if not self.is_first_line:
            self.contents.addch('\n')
        self.contents.addch("[")
        self.contents.addstr(self.label, curses.color_pair(self.color))
        self.contents.addch("]")
        self.contents.addch(" ")
        self.contents.addstr(str)
        self.is_first_line = False

    def refresh(self):
        self.contents.refresh()


class ProjectRunner:
    def __init__(self, target) -> None:
        self.output: Queue[str] = Manager().Queue()
        self.target = target
        self.built = False
        self.started = False

    def build(self):
        self.built = False

        def start_build(o: ProjectRunner, queue: Queue[str]):
            queue.put("Starting build..", False)
            subprocess.run(f"cargo build --quiet", cwd=o.target,
                           shell=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            o.built = True
            queue.put("Build finished!", False)

        self.build_t = threading.Thread(
            target=start_build, args=(self, self.output))
        self.build_t.daemon = True
        self.build_t.start()

    def start(self):
        if not self.built:
            raise ValueError("Not yet built!")
        self.proc = subprocess.Popen(f"target/debug/{self.target}", cwd=self.target, shell=True,
                                     stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        self.started = True

        def output_reader(queue: Queue[str], proc: subprocess.Popen[str]):
            while True:
                line = proc.stdout.readline()
                if line:
                    queue.put(line, False)

        self.t = threading.Thread(
            target=output_reader, args=(self.output, self.proc))
        self.t.daemon = True
        self.t.start()

    def terminate(self):
        self.proc.terminate()
        self.started = False

    def __del__(self):
        self.terminate()

    def rebuild(self):
        self.terminate()
        self.build()


class ProjectManager:
    def __init__(self, parent_scr: curses.window, height: int, width: int, y: int, x: int, color: int, label: str, target) -> None:
        self.win = TerminalWindow(
            parent_scr, height, width, y, x, color, label)
        self.proc = ProjectRunner(target)
        self.proc.build()

    def draw(self):
        if self.proc.built and not self.proc.started:
            self.proc.start()

        try:
            self.win.print(f"Value: {self.proc.output.get(False)}")
        except:
            pass

        self.win.refresh()


def main(stdscr: curses.window):
    curses.curs_set(0)
    stdscr.clear()
    stdscr.border()
    stdscr.nodelay(True)

    curses.init_pair(1, curses.COLOR_BLUE, curses.COLOR_BLACK)
    curses.init_pair(2, curses.COLOR_MAGENTA, curses.COLOR_BLACK)
    curses.init_pair(3, curses.COLOR_GREEN, curses.COLOR_BLACK)

    [height, width] = stdscr.getmaxyx()
    term_width = (width - 2) // 2

    azusa = ProjectManager(stdscr, height - 3, term_width,
                           y=1, x=1, color=1, label="Azusa", target="azusa")
    api = ProjectManager(stdscr, height - 3, term_width,
                         y=1, x=term_width + 1, color=3, label="API", target="api")

    while(True):
        try:
            ch = stdscr.getkey()
            if ch == 'q':
                break
            elif ch == 'w':
                api.proc.rebuild()
            elif ch == 'a':
                azusa.proc.rebuild()
        except:
            pass

        azusa.draw()
        api.draw()


curses.wrapper(main)
