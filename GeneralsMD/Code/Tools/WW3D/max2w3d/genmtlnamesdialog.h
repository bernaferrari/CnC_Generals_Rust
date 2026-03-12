#ifndef GENMTLNAMESDIALOG_H
#define GENMTLNAMESDIALOG_H

#include <windows.h>

class Interface;
class ISpinnerControl;

/**********************************************************************************************
**
** GenMtlNamesDialogClass - Dialog box for the material naming parameters
**
**********************************************************************************************/
class GenMtlNamesDialogClass
{
public:

	GenMtlNamesDialogClass(Interface * maxinterface);
	~GenMtlNamesDialogClass();
	
	enum 
	{
		MAX_MATERIAL_NAME_LEN	= 32,
		MIN_NAME_INDEX				= 0,
		MAX_NAME_INDEX				= 999,
		INITIAL_NAME_INDEX		= 0,
		MAX_ROOT_NAME_LEN			= 28,
	};

	struct OptionsStruct
	{
		OptionsStruct(void) : OnlyAffectSelected(true), NameIndex(0)
		{ 
			memset(RootName,0,sizeof(RootName)); 
		}
		
		// overall options		
		bool		OnlyAffectSelected;

		// name options
		char		RootName[MAX_MATERIAL_NAME_LEN];
		int		NameIndex;
	};

	bool Get_Options(OptionsStruct * options);
	bool Dialog_Proc(HWND hWnd,UINT message,WPARAM wParam,LPARAM);
	bool Ok_To_Exit(void);
		
private:

	HWND								Hwnd;

	OptionsStruct *				Options;
	Interface *						MaxInterface;
	ISpinnerControl *				NameIndexSpin;

	friend BOOL CALLBACK _gen_mtl_names_dialog_proc(HWND hwnd,UINT message,WPARAM wparam,LPARAM lparam);

};


#endif //GENMTLNAMESDIALOG_H


