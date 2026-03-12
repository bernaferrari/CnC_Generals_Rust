#include "monod.h"

MonoD::MonoD(void)
{
#ifdef _WIN32
  unsigned long retval;
  handle = CreateFile("\\\\.\\MONO", GENERIC_READ|GENERIC_WRITE, 0, NULL,
                        OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, NULL);

  if (handle != INVALID_HANDLE_VALUE)
  {
    DeviceIoControl(handle, (DWORD)IOCTL_MONO_CLEAR_SCREEN, NULL, 0, NULL, 0,
                     &retval,0);
  }
#endif
}

MonoD::~MonoD()
{
  #ifdef _WIN32
    CloseHandle(handle);
    handle=NULL;
  #endif
}

int MonoD::print(const char *str, int len)
{
  #ifdef _WIN32
    unsigned long retval;
    WriteFile(handle, str, len, &retval, NULL);
    //DeviceIoControl(handle, (DWORD)IOCTL_MONO_PRINT_RAW, (void *)str, len, NULL, 0,
    //                 &retval,0);
    return(len);
  #else
    for (int i=0; i<len; i++)
      fprintf(stderr,"%c",str[i]); 
    return(len);
  #endif
}
