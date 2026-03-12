#include "syslogd.h"

SyslogD::SyslogD(char *ident,int logopt,int facility,int _priority)
{
#ifndef _WINDOWS
  openlog(ident,logopt,facility);
  priority=_priority;
#endif
}

int SyslogD::print(const char *str, int len)
{
#ifndef _WINDOWS
  char *temp_str=new char[len+1];
  memset(temp_str,0,len+1);
  strncpy(temp_str,str,len);
  syslog(priority,temp_str);
  delete[](temp_str);
#endif
  return(len);
}
