/* Original Drift glue, MIT. Upstream engine sources remain unmodified.
 * The inherited stdin pipe is a lifetime lease, never user/traffic input.
 * If the owner crashes, EOF terminates the engine and releases its sockets. */
#include <stdlib.h>
#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#include <winsock2.h>
#include <windows.h>
static DWORD WINAPI watch_parent(LPVOID unused) {
    (void)unused; char buffer[16]; DWORD count;
    HANDLE input = GetStdHandle(STD_INPUT_HANDLE);
    while (ReadFile(input, buffer, sizeof(buffer), &count, NULL) && count != 0) {}
    ExitProcess(0); return 0;
}
#else
#include <pthread.h>
#include <unistd.h>
#include <errno.h>
static void *watch_parent(void *unused) {
    (void)unused; char buffer[16]; ssize_t count;
    do { count = read(STDIN_FILENO, buffer, sizeof(buffer)); } while (count > 0 || (count < 0 && errno == EINTR));
    _exit(0); return NULL;
}
#endif
int byedpi_main(int argc, char **argv);
int main(int argc, char **argv) {
#ifdef _WIN32
    HANDLE thread = CreateThread(NULL, 0, watch_parent, NULL, 0, NULL);
    if (!thread) return 1;
    CloseHandle(thread);
#else
    pthread_t thread;
    if (pthread_create(&thread, NULL, watch_parent, NULL) != 0) return 1;
    pthread_detach(thread);
#endif
    return byedpi_main(argc, argv);
}

/* Bind only outbound sockets to the pre-tunnel interface. This keeps the
 * fragmenting engine outside the TUN TCP stack, preserving wire segmentation. */
#if defined(__APPLE__)
#include <sys/socket.h>
#include <netinet/in.h>
#elif defined(_WIN32)
#include <ws2tcpip.h>
#elif defined(__linux__)
#include <sys/socket.h>
#include <net/if.h>
#endif
int drift_bind_outbound(int fd) {
    const char *text = getenv("DRIFT_OUTBOUND_IFINDEX");
    if (!text || !*text) return 0;
    char *end = NULL; unsigned long index = strtoul(text, &end, 10);
    if (!end || *end || index == 0 || index > 0xffffffffUL) return -1;
#if defined(__APPLE__)
    struct sockaddr_storage addr; socklen_t size = sizeof(addr);
    if (getsockname(fd, (struct sockaddr *)&addr, &size) < 0) return -1;
    unsigned int value = (unsigned int)index;
    return setsockopt(fd, addr.ss_family == AF_INET6 ? IPPROTO_IPV6 : IPPROTO_IP,
        addr.ss_family == AF_INET6 ? IPV6_BOUND_IF : IP_BOUND_IF, &value, sizeof(value));
#elif defined(_WIN32)
    DWORD v4 = htonl((DWORD)index), v6 = (DWORD)index;
    int a = setsockopt(fd, IPPROTO_IP, IP_UNICAST_IF, (char *)&v4, sizeof(v4));
    int b = setsockopt(fd, IPPROTO_IPV6, IPV6_UNICAST_IF, (char *)&v6, sizeof(v6));
    return a == 0 || b == 0 ? 0 : -1;
#elif defined(__linux__)
    char name[IF_NAMESIZE];
    if (!if_indextoname((unsigned int)index, name)) return -1;
    return setsockopt(fd, SOL_SOCKET, SO_BINDTODEVICE, name, sizeof(name));
#else
    (void)fd; return -1;
#endif
}
