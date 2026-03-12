#ifndef GAMEMTLTEXTUREDLG_H
#define GAMEMTLTEXTUREDLG_H

#include <Max.h>
#include "GameMtlForm.h"

class GameMtl;

class GameMtlTextureDlg : public GameMtlFormClass
{

public:

	GameMtlTextureDlg(HWND parent, IMtlParams * imp, GameMtl * m, int pass);
	~GameMtlTextureDlg(void);

	virtual BOOL		Dialog_Proc (HWND dlg_wnd, UINT message, WPARAM wparam, LPARAM lparam);
	void					ActivateDlg(BOOL onOff);
	void					ReloadDialog(void);

private:
	
	void					Enable_Stage(int stage,BOOL onoff);
	void					Update_Texture_Buttons(void);

	ISpinnerControl * Stage0FramesSpin;
	ISpinnerControl * Stage1FramesSpin;

	ISpinnerControl * Stage0RateSpin;
	ISpinnerControl * Stage1RateSpin;

	ICustButton *		Stage0PublishButton;
	ICustButton *		Stage1PublishButton;
	ICustButton *		Stage0ClampUButton;
	ICustButton *		Stage1ClampUButton;
	ICustButton *		Stage0ClampVButton;
	ICustButton *		Stage1ClampVButton;
	ICustButton *		Stage0NoLODButton;
	ICustButton *		Stage1NoLODButton;
	ICustButton *		Stage0AlphaBitmapButton;
	ICustButton *		Stage1AlphaBitmapButton;
	ICustButton *		Stage0DisplayButton;
	ICustButton *		Stage1DisplayButton;
};




#endif


