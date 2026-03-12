#include "floaterdialog.h"
#include "dllmain.h"
#include "resource.h"
#include <Max.h>


/**********************************************************************************************
**
** FloaterDialogClass Implementation
**
**********************************************************************************************/

BOOL CALLBACK _floater_dialog_proc(HWND hwnd,UINT message,WPARAM wParam,LPARAM lParam)
{
	if (message == WM_INITDIALOG) {
		FloaterDialogClass * floater = (FloaterDialogClass *)lParam;
		::SetProp(hwnd,"FloaterDialogClass",(HANDLE)floater);
	}

	FloaterDialogClass * floater = (FloaterDialogClass *)::GetProp(hwnd,"FloaterDialogClass");

	if (message == WM_DESTROY) {
		::RemoveProp(hwnd,"FloaterDialogClass");
	}


	if (floater) {
		return floater->Dialog_Proc(hwnd,message,wParam,lParam);
	} else {
		return FALSE;
	}
}


/***********************************************************************************************
 * FloaterDialogClass::FloaterDialogClass -- Constructor                                       *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *=============================================================================================*/
FloaterDialogClass::FloaterDialogClass(void) :
	Hwnd(NULL),
	ChildDialogTemplateID(-1),
	ChildDialogProc(NULL)
{
}


/***********************************************************************************************
 * FloaterDialogClass::~FloaterDialogClass -- Destructor                                       *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *=============================================================================================*/
FloaterDialogClass::~FloaterDialogClass(void)
{
	if (Hwnd != NULL) {
		::DestroyWindow(Hwnd);
	}
}


/***********************************************************************************************
 * FloaterDialogClass::Is_Created -- test whether the floater has already been created         *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   10/11/2000 gth : Created.                                                                 *
 *=============================================================================================*/
bool FloaterDialogClass::Is_Created(void)
{
	return (Hwnd != NULL);
}


/***********************************************************************************************
 * FloaterDialogClass::Create -- create the window                                             *
 *                                                                                             *
 *    This function will return automatically if the floater has been created already.         *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   10/11/2000 gth : Created.                                                                 *
 *=============================================================================================*/
void FloaterDialogClass::Create(Interface * ip, int child_dlg_id, DLGPROC child_dlg_proc)
{
	/*
	** Don't create multiple ones
	*/
	if (Is_Created()) {
		return;
	}

	/*
	** Copy down the data needed to create the child window later
	*/
	ChildDialogTemplateID = child_dlg_id;
	ChildDialogProc = child_dlg_proc;


	/*
	** Create the dialog box
	*/
	Hwnd = CreateDialogParam(	
										AppInstance,
										MAKEINTRESOURCE(IDD_W3DUTILITY_FLOATER_DIALOG),
										::GetCOREInterface()->GetMAXHWnd(),
										(DLGPROC) _floater_dialog_proc,
										(LPARAM) this 
									);
	::GetCOREInterface()->RegisterDlgWnd(Hwnd); 
}
	


/***********************************************************************************************
 * FloaterDialogClass::Dialog_Proc -- Dialog Proc for the floater                              *
 *                                                                                             *
 * The only thing we need to do here is to create the child dialog and resize ourselves to     *
 * contain it.                                                                                 *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   10/11/2000 gth : Created.                                                                 *
 *=============================================================================================*/
bool FloaterDialogClass::Dialog_Proc(HWND hWnd,UINT message,WPARAM wParam,LPARAM)
{
	switch (message )	{

		case WM_INITDIALOG:
			{
				HWND childhwnd = CreateDialogParam(	
													AppInstance,
													MAKEINTRESOURCE(ChildDialogTemplateID),
													hWnd,
													ChildDialogProc,
													0
												);
				if (childhwnd!= NULL) {
					RECT rect;
					LONG style = ::GetWindowLong(hWnd,GWL_STYLE);
					::GetWindowRect(childhwnd,&rect);
					::AdjustWindowRect(&rect,style,FALSE);
					::SetWindowPos(hWnd,NULL,0,0,rect.right - rect.left,rect.bottom - rect.top,SWP_NOZORDER|SWP_NOMOVE);
					::SetWindowPos(childhwnd,NULL,0,0,0,0,SWP_NOZORDER|SWP_NOSIZE|SWP_SHOWWINDOW);
				}
			}
			return 1;
		
		case WM_COMMAND:
			switch (LOWORD(wParam))
			{
				case IDCANCEL:
					DestroyWindow(Hwnd);
					break;
			}
			return 1;

		case WM_DESTROY:
			::GetCOREInterface()->UnRegisterDlgWnd(Hwnd); 
			Hwnd = NULL;
			break;
	}
	return 0;
}
