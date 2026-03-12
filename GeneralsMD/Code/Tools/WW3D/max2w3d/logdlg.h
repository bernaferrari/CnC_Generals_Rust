#ifndef LOGDLG_H
#define LOGDLG_H

#include <windows.h>


class LogDataDialogClass
{
public:

	LogDataDialogClass(HWND parent);
	~LogDataDialogClass();
	
   void	Wait_OK();	// wait for user to hit OK
   
   void	printf(char *, ...);
	void	printf(char * text, const va_list & args);
	void  rprintf(char *, ...);
	void	rprintf(char *text, const va_list & args);
	
	void	updatebar(float position, float total);
   
	bool	Dialog_Proc(HWND hWnd,UINT message,WPARAM wParam,LPARAM);

public:
// public variables
	HWND		Hwnd;
	HWND		ParentHwnd;

private:

	void Dialog_Init();

private:

	HANDLE	ThreadHandle;
	DWORD		ThreadID;

	int	last_buffer_index;
	int	buffer_index;

volatile int status;
  
};


#endif

// EOF - logdlg.h
