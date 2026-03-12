// timingTest.cpp : Defines the entry point for the console application.
//

#include "stdafx.h"
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <mmsystem.h>
#include <iostream.h>
#include <string.h>

double s_ticksPerSec = 0.0f;
double s_ticksPerMSec = 0.0f;
char buffer[1024];

//-------------------------------------------------------------------------------------------------
void GetPrecisionTimer(INT64* t)
{
	// CPUID is needed to force serialization of any previous instructions. 
	__asm 
	{
		RDTSC
		MOV ECX,[t]
		MOV [ECX], EAX
		MOV [ECX+4], EDX
	}
}

//-------------------------------------------------------------------------------------------------
void InitPrecisionTimer()
{
	__int64 totalTime = 0;
	INT64	TotalTicks = 0;
	static int TESTS = 10;

	cout << "Starting tests..." << flush;
	
	for (int i = 0; i < TESTS; ++i) 
	{
		int            TimeStart;
		int            TimeStop;
		INT64		   StartTicks;
		INT64		   EndTicks;

		TimeStart = timeGetTime();
		GetPrecisionTimer(&StartTicks);
		for(;;)
		{
			TimeStop = timeGetTime();
			if ((TimeStop - TimeStart) > 1000)
			{
				GetPrecisionTimer(&EndTicks);
				break;
			}
		}

		TotalTicks += (EndTicks - StartTicks);

		totalTime += (TimeStop - TimeStart);
	}

	cout << "...completed" << endl;
	s_ticksPerMSec = 1.0 * TotalTicks / totalTime;
	s_ticksPerSec = s_ticksPerMSec * 1000.0f;

	sprintf(buffer, "Ticks per sec: %.2f\n", s_ticksPerSec);
	cout << buffer;
	sprintf(buffer, "Ticks per msec: %.2f\n", s_ticksPerMSec);
	cout << buffer;
}

int main(int argc, char* argv[])
{
	INT64 startTime, endTime, totalTime = 0;
	InitPrecisionTimer();
	FILE *out = fopen("output.txt", "w");
	cout << "Beginning Looping tests: " << endl;
	
	const int TESTCOUNT = 60;

	while (1) {
		for (int i = 0; i < TESTCOUNT; ++i) {
			GetPrecisionTimer(&startTime);
			Sleep(5);
			GetPrecisionTimer(&endTime);
			totalTime += (endTime - startTime);	
		}

		double avgPerFrame = 1.0 * totalTime / TESTCOUNT;

		sprintf(buffer, "%.8f,\t", avgPerFrame / s_ticksPerMSec );	
		fwrite(buffer, strlen(buffer), 1, out);
		fflush(out);
		cout << buffer << endl;
		totalTime = 0;
	}
	fclose(out);

	return 0;
}

