#include "process.h"

Process::Process()
{ 
  directory[0]=0;
  command[0]=0;
  args[0]=0;
  hProcess=NULL;
  hThread=NULL;
}

// Create a process
bit8 Create_Process(Process &process)
{
    int                      retval;
    STARTUPINFO              si;
    PROCESS_INFORMATION      piProcess;
    ZeroMemory(&si,sizeof(si));
    si.cb=sizeof(si);
 
    char cmdargs[513];
    memset(cmdargs,0,513);
    strcpy(cmdargs,process.command);
    strcat(cmdargs,process.args);

    retval=CreateProcess(NULL,cmdargs,NULL,NULL,FALSE, 0  ,NULL, NULL/*process.directory*/,&si,&piProcess);

    process.hProcess=piProcess.hProcess;
    process.hThread=piProcess.hThread;
    return(TRUE);
}

//
// Wait for a process to complete, and fill in the exit code
//
bit8 Wait_Process(Process &process, DWORD *exit_code)
{
  DWORD retval;
  retval=WaitForSingleObject(process.hProcess,INFINITE);
  if (exit_code != NULL)
    *exit_code=-1;
  if (retval==WAIT_OBJECT_0)  // process exited
  {
    if (exit_code != NULL)
      GetExitCodeProcess(process.hProcess,exit_code);
    return(TRUE);
  }
  else                        // can this happen?
    return(FALSE);
}

/*******************
//
// Get the process to run from the config object
//
bit8 Read_Process_Info(ConfigFile &config,OUT Process &info)
{

 Wstring procinfo;
 if (config.getString("RUN",procinfo)==FALSE)
 {
   DBGMSG("Couldn't read the RUN line");
   return(FALSE);
 }
 int          offset=0;
 Wstring      dir;
 Wstring      executable;
 Wstring      args;
 offset=procinfo.getToken(offset," ",dir);
 offset=procinfo.getToken(offset," ",executable);
 args=procinfo;
 args.remove(0,offset);

 ///
 ///
 DBGMSG("RUN: EXE = "<<executable.get()<<"  DIR = "<<dir.get()<<
   "  ARGS = "<<args.get());
 strcpy(info.command,executable.get());
 strcpy(info.directory,dir.get());
 strcpy(info.args,args.get());
 return(TRUE);
}
*****************/
