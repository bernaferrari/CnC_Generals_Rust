#ifndef FILED_HEADER
#define FILED_HEADER

#include "odevice.h"

class FileD : public OutputDevice
{
 public:
   FileD(IN char *filename, IN char *mode = "w")
   { 
     out=fopen(filename,mode);
     if (out==NULL)
       out=fopen("FileDev.out",mode);
   }

   virtual ~FileD()
   { fclose(out); }

   virtual int print(const char *str,int len)
   {
     char *string=new char[len+1];
     memset(string,0,len+1);
     memcpy(string,str,len);
     fprintf(out,"%s",string);
     delete[](string);
     fflush(out);
     return(len);
   }

   FILE      *out;
};

#endif
