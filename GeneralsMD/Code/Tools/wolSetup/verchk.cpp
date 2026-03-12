#include "verchk.h"
#include "wolsetup.h"
#include <windows.h>
#include <winnt.h>
#include <stdlib.h>

/**
	* Retrieve version information from files version resource.
	*
	* INPUTS
	*     Filename - Name of file to retrieve version information for.
	*     FileInfo - Pointer to VS_FIXEDFILEINFO structure to be filled in.
	*
	* RESULT
	*     Success - True if successful in obtaining version information.
	*/
bool GetVersionInfo(char* filename, VS_FIXEDFILEINFO* fileInfo)
{
   if (filename == NULL || fileInfo == NULL)
   {
      return false;
   }

   // Get version information from the application
   DWORD verHandle;
   DWORD verInfoSize = GetFileVersionInfoSize(filename, &verHandle);

   if (verInfoSize)
   {
      // If we were able to get the information, process it:
      HANDLE memHandle = GlobalAlloc(GMEM_MOVEABLE, verInfoSize);

      if (memHandle)
      {
         LPVOID buffer = GlobalLock(memHandle);

         if (buffer)
         {
            BOOL success = GetFileVersionInfo(filename, verHandle, verInfoSize, buffer);

            if (success)
            {
               VS_FIXEDFILEINFO* data;
               UINT dataSize = 0;
               success = VerQueryValue(buffer, "\\", (LPVOID*) & data, &dataSize);

               if (success && (dataSize == sizeof(VS_FIXEDFILEINFO)))
               {
                  memcpy(fileInfo, data, sizeof(VS_FIXEDFILEINFO));
                  return true;
               }
            }

            GlobalUnlock(memHandle);
         }

         GlobalFree(memHandle);
      }
   }

   return false;
}

bool loadWolapi( char *filename )
{
	VS_FIXEDFILEINFO fileInfo;
	if (GetVersionInfo(filename, &fileInfo))
		g_wolapiInstalled = false;

	return g_wolapiInstalled;
}
