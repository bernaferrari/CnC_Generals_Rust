#ifndef MONOD_HEADER
#define MONOD_HEADER


#include <stdlib.h>
#include <stdio.h>
#include "odevice.h"

///////////////////////// WIN32 ONLY ///////////////////////////////////

#ifdef _WIN32
#include <windows.h>
#include <winioctl.h>


/*
**  This is the identifier for the Monochrome Display Driver
*/
#define FILE_DEVICE_MONO 0x00008000

/*
**      These are the IOCTL commands supported by the Monochrome Display Driver.
*/
#define IOCTL_MONO_HELP_SCREEN  CTL_CODE(FILE_DEVICE_MONO, 0x800, METHOD_BUFFERED, FILE_WRITE_DATA)
#define IOCTL_MONO_CLEAR_SCREEN CTL_CODE(FILE_DEVICE_MONO, 0x801, METHOD_BUFFERED, FILE_WRITE_DATA)
#define IOCTL_MONO_PRINT_RAW            CTL_CODE(FILE_DEVICE_MONO, 0x802, METHOD_BUFFERED, FILE_WRITE_DATA)
#define IOCTL_MONO_SET_CURSOR           CTL_CODE(FILE_DEVICE_MONO, 0x803, METHOD_BUFFERED, FILE_WRITE_DATA)
#define IOCTL_MONO_SCROLL                       CTL_CODE(FILE_DEVICE_MONO, 0x804, METHOD_BUFFERED, FILE_WRITE_DATA)
#define IOCTL_MONO_BRING_TO_TOP CTL_CODE(FILE_DEVICE_MONO, 0x805, METHOD_BUFFERED, FILE_WRITE_DATA)
#define IOCTL_MONO_SET_ATTRIBUTE        CTL_CODE(FILE_DEVICE_MONO, 0x806, METHOD_BUFFERED, FILE_WRITE_DATA)
#define IOCTL_MONO_PAN                          CTL_CODE(FILE_DEVICE_MONO, 0x807, METHOD_BUFFERED, FILE_WRITE_DATA)
#define IOCTL_MONO_LOCK                         CTL_CODE(FILE_DEVICE_MONO, 0x808, METHOD_BUFFERED, FILE_WRITE_DATA)
#define IOCTL_MONO_UNLOCK                       CTL_CODE(FILE_DEVICE_MONO, 0x809, METHOD_BUFFERED, FILE_WRITE_DATA)
#define IOCTL_MONO_SET_WINDOW           CTL_CODE(FILE_DEVICE_MONO, 0x80A, METHOD_BUFFERED, FILE_WRITE_DATA)
#define IOCTL_MONO_RESET_WINDOW CTL_CODE(FILE_DEVICE_MONO, 0x80B, METHOD_BUFFERED, FILE_WRITE_DATA)
#define IOCTL_MONO_SET_FLAG             CTL_CODE(FILE_DEVICE_MONO, 0x80C, METHOD_BUFFERED, FILE_WRITE_DATA)
#define IOCTL_MONO_CLEAR_FLAG           CTL_CODE(FILE_DEVICE_MONO, 0x80D, METHOD_BUFFERED, FILE_WRITE_DATA)
#define IOCTL_MONO_FILL_ATTRIB  CTL_CODE(FILE_DEVICE_MONO, 0x80E, METHOD_BUFFERED, FILE_WRITE_DATA)

#endif  // ifdef _WIN32

class MonoD : public OutputDevice
{
 public:
                   MonoD();
                  ~MonoD();

   virtual int    print(const char *str,int len);

 private:
   #ifdef _WIN32
     HANDLE handle;
   #endif
};

#endif
