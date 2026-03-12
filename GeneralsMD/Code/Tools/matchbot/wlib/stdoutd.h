#ifndef STDOUTD_HEADER
#define STDOUTD_HEADER

#include "odevice.h"

class StdoutD : public OutputDevice
{
 public:

   virtual int print(const char *str,int len)
   {
     char *string=new char[len+1];
     memcpy(string,str,len);
     string[len]=0;
     fprintf(stdout,"%s",string);
     fflush(stdout);
     delete[](string);
     return(len);
   }
};

#endif
