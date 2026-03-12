#ifndef W3DDLG_H
#define W3DDLG_H

#include "always.h"
#include <Max.h>
#include "w3dutil.h"


class W3dOptionsDialogClass
{
public:

	W3dOptionsDialogClass(Interface * maxinterface,ExpInterface * exportinterface);
	~W3dOptionsDialogClass();
	
	bool Get_Export_Options(W3dExportOptionsStruct * options);
	bool Dialog_Proc(HWND hWnd,UINT message,WPARAM wParam,LPARAM);

public:

	HWND								Hwnd;

private:

	void Dialog_Init();
	BOOL Dialog_Ok();
	void Enable_WHT_Export();
	void Enable_WHT_Load();
	void Disable_WHT_Export();
	void Enable_WHA_Export();
	void Disable_WHA_Export();
	void Enable_WTM_Export();
	void Disable_WTM_Export();
	
	void Enable_ReduceAnimationOptions_Export();
	void Disable_ReduceAnimationOptions_Export();
	void Enable_CompressAnimationOptions_Export();
	void Disable_CompressAnimationOptions_Export();
	
	void WHT_Export_Radio_Changed();
	void WHA_Export_Radio_Changed();
	void WTM_Export_Radio_Changed();

	void WHA_Compress_Animation_Check_Changed();
	void WHA_Reduce_Animation_Check_Changed();

	void WHA_Compression_Flavor_Changed();

private:

	W3dExportOptionsStruct *	Options;
	bool								GotHierarchyFilename;
	Interface *						MaxInterface;
	ExpInterface *		 			ExportInterface;

	ISpinnerControl *				RangeLowSpin;
	ISpinnerControl *				RangeHighSpin;
	
	HWND								HwndReduce;
	HWND								HwndFlavor;
	HWND								HwndTError;
	HWND								HwndRError;
  
	int								UnitsType;
	float								UnitsScale;
};


#endif