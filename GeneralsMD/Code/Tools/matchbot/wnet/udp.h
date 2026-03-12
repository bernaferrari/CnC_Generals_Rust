#ifndef UDP_HEADER
#define UDP_HEADER

#include <stdio.h>
#include <stdlib.h>
#include <ctype.h>
#include <errno.h>
#include <string.h>

#ifdef _WINDOWS
#include <winsock.h>
#include <io.h>
#define close _close
#define read  _read
#define write _write

#else  //UNIX
#include <netdb.h>
#include <sys/types.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <unistd.h>
#include <sys/time.h>
#include <fcntl.h>
#include <limits.h>
#endif

#ifdef AIX
#include <sys/select.h>
#endif

#define DEFAULT_PROTOCOL 0

#include <wlib/wstypes.h>
#include <wlib/wtime.h>

class UDP
{
 // DATA
 private:
  sint32       fd; 
  uint32       myIP;
  uint16       myPort;
  struct       sockaddr_in  addr;
  
  // These defines specify a system independent way to
  //   get error codes for socket services.
  enum sockStat
  {
    OK           =  0,     // Everything's cool
    UNKNOWN      = -1,     // There was an error of unknown type
    ISCONN       = -2,     // The socket is already connected
    INPROGRESS   = -3,     // The socket is non-blocking and the operation
                           //   isn't done yet
    ALREADY      = -4,     // The socket is already attempting a connection
                           //   but isn't done yet
    AGAIN        = -5,     // Try again.
    ADDRINUSE    = -6,     // Address already in use
    ADDRNOTAVAIL = -7,     // That address is not available on the remote host
    BADF         = -8,     // Not a valid FD
    CONNREFUSED  = -9,     // Connection was refused
    INTR         =-10,     // Operation was interrupted
    NOTSOCK      =-11,     // FD wasn't a socket
    PIPE         =-12,     // That operation just made a SIGPIPE
    WOULDBLOCK   =-13,     // That operation would block
    INVAL        =-14,     // Invalid
    TIMEDOUT     =-15      // Timeout
  };

// CODE
 private:
  sint32           SetBlocking(bit8 block);

 public:
                   UDP();
                  ~UDP();
  sint32           Bind(uint32 IP,uint16 port);
  sint32           Bind(char *Host,uint16 port);
  sint32           Write(uint8 *msg,uint32 len,uint32 IP,uint16 port);
  sint32           Read(uint8 *msg,uint32 len,sockaddr_in *from);
  sockStat         GetStatus(void);
  void             ClearStatus(void);
  int              Wait(sint32 sec,sint32 usec,fd_set &returnSet);
  int              Wait(sint32 sec,sint32 usec,fd_set &givenSet,fd_set &returnSet);

  bit8             getLocalAddr(uint32 &ip, uint16 &port);
  sint32           getFD(void) { return(fd); }
 
  bit8             SetInputBuffer(uint32 bytes);
  bit8             SetOutputBuffer(uint32 bytes);
  int              GetInputBuffer(void);
  int              GetOutputBuffer(void);
};

#endif
