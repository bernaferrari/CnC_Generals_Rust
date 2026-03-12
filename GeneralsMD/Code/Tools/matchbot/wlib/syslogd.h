#ifndef SYSLOGD_HEADER
#define SYSLOGD_HEADER

#include <stdlib.h>
#include <stdio.h>
#ifndef _WINDOWS
#include <syslog.h>
#endif
#include <string.h>

// Windows headers have a tendency to redefine IN
#ifdef IN
#undef IN
#endif
#define IN const

#include "odevice.h"

// Windows doesn't have a syslog equivalent (does it?), so this class does little there
class SyslogD : public OutputDevice
{
 public:
   SyslogD(char *ident,int logopt,int facility,int priority);
   virtual int print(const char *str,int len);

 private:
   int priority;
};

#endif
