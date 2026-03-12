#ifndef GENLODEXTENSIONDIALOG_H
#define GENLODEXTENSIONDIALOG_H

#include <windows.h>

class Interface;
class ISpinnerControl;

/**********************************************************************************************
**
** GenLodExtensionDialogClass - Dialog box for the LOD extension naming parameters
**
**********************************************************************************************/
class GenLodExtensionDialogClass
{
public:

	GenLodExtensionDialogClass(Interface * maxinterface);
	~GenLodExtensionDialogClass();
	
	struct OptionsStruct
	{
		OptionsStruct(void) : LodIndex(0)
		{ 
		}
		
		// name options
		int		LodIndex;
	};

	bool Get_Options(OptionsStruct * options);
	bool Dialog_Proc(HWND hWnd,UINT message,WPARAM wParam,LPARAM);
		
private:

	enum 
	{
		MIN_LOD_INDEX				= 0,
		MAX_LOD_INDEX				= 99,
		INITIAL_LOD_INDEX			= 0,
	};

	HWND								Hwnd;

	OptionsStruct *				Options;
	Interface *						MaxInterface;
	ISpinnerControl *				LodIndexSpin;

	friend BOOL CALLBACK			_gen_lod_ext_dialog_proc(HWND Hwnd,UINT message,WPARAM wParam,LPARAM lParam);

};




#endif //GENLODEXTENSIONDIALOG_H