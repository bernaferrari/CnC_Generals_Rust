#ifndef MBOXD_HEADER
#define MBOXD_HEADER

#include "odevice.h"

class MboxD : public OutputDevice
{
 public:

   virtual int print(const char *str,int len)
   {
     char *string=new char[len+1];
     memset(string,0,len+1);
     memcpy(string,str,len);
     MessageBox(NULL,string,"Debug Message", MB_OK | MB_ICONINFORMATION);
     delete[](string);
     return(len);
   }

};

#endif
