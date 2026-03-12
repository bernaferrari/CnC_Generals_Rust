#ifndef STDERRD_HEADER
#define STDERRD_HEADER

#include "odevice.h"

class StderrD : public OutputDevice
{
 public:

   virtual int print(const char *str,int len)
   {
     char *string=new char[len+1];
     memset(string,0,len+1);
     memcpy(string,str,len);
     fprintf(stderr,"%s",string);
     delete[](string);
     return(len);
   }
};

#endif
