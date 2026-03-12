#ifndef __GLOBAL_H__
#define __GLOBAL_H__

#ifdef _WIN32
#include <process.h>
#ifdef IN
#undef IN
#endif
#define IN const
#endif
#include <wstypes.h>
#include <configfile.h>
#include <critsec.h>
#include <threadfac.h>
#include <tcp.h>
#include "matcher.h"
#include "rand.h"

class GlobalClass
{
public:
	GlobalClass();

	ConfigFile config;
	bool ReadFile(const char *fname);

	bool GetString(const Wstring& key, Wstring& val);

	RandClass rnd;
};

extern GlobalClass Global;

// Log rotation functions
void rotateOutput(void);
void rotateParanoid(void);

#endif

