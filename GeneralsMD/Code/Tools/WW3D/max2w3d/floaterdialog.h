#ifndef FLOATERDIALOG_H
#define FLOATERDIALOG_H

#include <windows.h>

class Interface;

/**
** FloaterDialogClass
** This class is designed to be used by modeless dialog boxes.  See w3dutil.cpp for an
** example of how to embed an arbitrary dialog template and dialog proc into a floating
** window.
*/
class FloaterDialogClass
{
public:

	FloaterDialogClass(void);
	~FloaterDialogClass();
	
	bool	Is_Created(void);
	void	Create(Interface * ip, int child_dialog_id, DLGPROC child_dlg_proc);
	bool	Dialog_Proc(HWND hWnd,UINT message,WPARAM wParam,LPARAM);

private:

	HWND		Hwnd;
	int		ChildDialogTemplateID;
	DLGPROC	ChildDialogProc;

};



#endif //FLOATERDIALOG_H

