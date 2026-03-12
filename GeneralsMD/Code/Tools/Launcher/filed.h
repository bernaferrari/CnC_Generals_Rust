#ifndef FILED_HEADER
#define FILED_HEADER

#include "odevice.h"

class FileD : public OutputDevice
{
 public:
   FileD(char *filename, bool outputDebug) : m_outputDebug(outputDebug)
   { 
     out=fopen(filename,"w");
     if (out==NULL)
       out=fopen("FileDev.out","w");
   }

   virtual ~FileD()
   { fclose(out); }

   virtual int print(const char *str,int len)
   {
     char *string=new char[len+1];
     memset(string,0,len+1);
     memcpy(string,str,len);
     fprintf(out,"%s",string);
		 if (m_outputDebug)
		 {
			 OutputDebugString(string);
		 }
     delete[](string);
     fflush(out);
     return(len);
   }

   FILE      *out;
	 bool m_outputDebug;
};

#endif
