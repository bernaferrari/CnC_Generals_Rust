#include "genlodextensiondialog.h"
#include "dllmain.h"
#include "resource.h"
#include <Max.h>


/**********************************************************************************************
**
** GenLodExtensionDialogClass Implementation
**
**********************************************************************************************/


/***********************************************************************************************
 * GenLodExtensionDialogClass::GenLodExtensionDialogClass -- Constructor                       *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *=============================================================================================*/
GenLodExtensionDialogClass::GenLodExtensionDialogClass(Interface * maxinterface) :
	Hwnd(NULL),
	Options(NULL),
	MaxInterface(maxinterface),
	LodIndexSpin(NULL)
{
}


/***********************************************************************************************
 * GenLodExtensionDialogClass::~GenLodExtensionDialogClass -- Destructor                       *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   10/10/2000 gth : Created.                                                                 *
 *=============================================================================================*/
GenLodExtensionDialogClass::~GenLodExtensionDialogClass(void)
{
	ReleaseISpinner(LodIndexSpin);
}


/***********************************************************************************************
 * GenLodExtensionDialogClass::Get_Options -- Presents the dialog, gets user input             *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   10/10/2000 gth : Created.                                                                 *
 *=============================================================================================*/
bool GenLodExtensionDialogClass::Get_Options(OptionsStruct * options)
{
	Options = options;

	// Put up the options dialog box.
	BOOL result = DialogBoxParam
						(
							AppInstance,
							MAKEINTRESOURCE (IDD_GENERATE_LOD_EXTENSION_DIALOG),
							MaxInterface->GetMAXHWnd(),
							(DLGPROC) _gen_lod_ext_dialog_proc,
							(LPARAM) this
						);

	if (result == TRUE) {
		return true;
	} else {
		return false;
	}
}


/***********************************************************************************************
 * GenLodExtensionDialogClass::Dialog_Proc -- Windows message handling                         *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   10/10/2000 gth : Created.                                                                 *
 *=============================================================================================*/
bool GenLodExtensionDialogClass::Dialog_Proc(HWND hWnd,UINT message,WPARAM wParam,LPARAM)
{
	switch (message )	{

		case WM_INITDIALOG:

			// Setup the LOD spinner control.
			LodIndexSpin = SetupIntSpinner
			(
				Hwnd,
				IDC_LOD_INDEX_SPIN,
				IDC_LOD_INDEX_EDIT,
				MIN_LOD_INDEX,MAX_LOD_INDEX,INITIAL_LOD_INDEX
			);
			
			return 1;

		case WM_COMMAND:

			switch (LOWORD(wParam))
			{
				case IDOK:
					Options->LodIndex = LodIndexSpin->GetIVal();
					EndDialog(Hwnd, 1);
					break;

				case IDCANCEL:
					EndDialog(Hwnd, 0);
					break;
			}
			return 1;
	}
	return 0;
}


/***********************************************************************************************
 * _gen_lod_ext_dialog_proc -- windows dialog proc                                             *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   10/10/2000 gth : Created.                                                                 *
 *=============================================================================================*/
static BOOL CALLBACK _gen_lod_ext_dialog_proc(HWND hwnd,UINT message,WPARAM wparam,LPARAM lparam)
{
	static GenLodExtensionDialogClass * dialog = NULL;

	if (message == WM_INITDIALOG) {
		dialog = (GenLodExtensionDialogClass *)lparam;
		dialog->Hwnd = hwnd;
	}

	if (dialog) {
		return dialog->Dialog_Proc(hwnd, message, wparam, lparam);
	} else {
		return FALSE;
	}
}




