#ifndef PROCESS_HEADER
#define PROCESS_HEADER

#include <windows.h>
#include "wstypes.h"
#include "wdebug.h"
#include "configfile.h"

class Process
{
 public:
           Process();

  char     directory[256];
  char     command[256];
  char     args[256];
  HANDLE   hProcess;
	DWORD dwProcessID;
  HANDLE   hThread;
	DWORD dwThreadID;
};

bit8 Read_Process_Info(ConfigFile &config,OUT Process &info, IN char *key = NULL);
bit8 Create_Process(Process &process);
bit8 Wait_Process(Process &process, DWORD *exit_code=NULL);


#endif