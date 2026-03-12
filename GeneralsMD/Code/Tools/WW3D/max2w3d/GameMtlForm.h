#ifndef GAMEMTLFORM_H
#define GAMEMTLFORM_H

#include "FormClass.h"

class GameMtl;

class GameMtlFormClass : public FormClass
{
public:
	GameMtlFormClass(IMtlParams * imtl_params,GameMtl * mtl,int pass);

	void					SetThing(ReferenceTarget *m);
	ReferenceTarget*	GetThing(void);
	void					DeleteThis(void);
	Class_ID				ClassID(void);
	void					SetTime(TimeValue t);

protected:

	IMtlParams *		IParams;			// interface to the material editor
	GameMtl *			TheMtl;			// current mtl being edited.
	int					PassIndex;		// material pass that this form edits
};

#endif
