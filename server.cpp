#include <fcntl.h>
#include <netinet/in.h>
#include <sys/epoll.h>
#include <sys/socket.h>
#include <unistd.h>

#include <iostream>

using namespace std;

int main() {
  const int kEVENT_SIZE = 20;
  char buff[1024];
  int listen_fd = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
  sockaddr_in sock_addr;
  sock_addr.sin_port = htons(8088);
  sock_addr.sin_family = AF_INET;
  sock_addr.sin_addr.s_addr = htons(INADDR_ANY);

  if (bind(listen_fd, (sockaddr*)&sock_addr, sizeof(sock_addr)) == -1) {
    cout << "bind error" << endl;
    return -1;
  }

  if (listen(listen_fd, 1024) == -1) {
    cout << "listen error" << endl;
    return -1;
  }

  int epoll_fd = epoll_create(1024);
  epoll_event event{};
  event.events = EPOLLIN;
  event.data.fd = listen_fd;
  epoll_ctl(epoll_fd, EPOLL_CTL_ADD, listen_fd, &event);
  epoll_event events[kEVENT_SIZE];

  while (true) {
    int n = epoll_wait(epoll_fd, events, kEVENT_SIZE, -1);
    if (n == -1) {
      cout << "epoll_wait" << endl;
      return -1;
    }
    for (int i = 0; i < n; ++i) {
      if (events[i].data.fd == listen_fd) {
        if (events[i].events & EPOLLIN) {
          sockaddr_in client_addr{};
          socklen_t len = sizeof(client_addr);
          int f = accept(listen_fd, (sockaddr*)&client_addr, &len);
          if (f > 0) {
            event.events = EPOLLIN | EPOLLET;
            event.data.fd = f;
            int flags = fcntl(f, F_GETFL, 0);
            if (flags < 0) {
              cout << "set no block error, fd:" << epoll_fd << endl;
              continue;
            }
            if (fcntl(f, F_SETFL, flags | O_NONBLOCK) < 0) {
              cout << "set no block error, fd:" << epoll_fd << endl;
              continue;
            }
            epoll_ctl(epoll_fd, EPOLL_CTL_ADD, f, &event);
            cout << "client on line fd:" << epoll_fd << endl;
          }
        }
      } else {
        if (events[i].events & EPOLLERR || events[i].events & EPOLLHUP) {
          epoll_ctl(epoll_fd, EPOLL_CTL_DEL, events[i].data.fd, nullptr);
          cout << "client out fd:" << events[i].data.fd << endl;
          close(events[i].data.fd);
        } else if (events[i].events & EPOLLIN) {
          int len = read(events[i].data.fd, buff, sizeof(buff));
          if (len == -1) {
            epoll_ctl(epoll_fd, EPOLL_CTL_DEL, events[i].data.fd, nullptr);
            cout << "client out fd:" << events[i].data.fd << endl;
            close(events[i].data.fd);
          } else {
            cout << buff << endl;
            char a[] = "1234567";
            write(events[i].data.fd, a, sizeof(a));
          }
        }
      }
    }
  }
  return 0;
}
