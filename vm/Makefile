# all commands
.DEFAULT_GOAL := all
.PHONY: all clean run test

CC := gcc
PYTHON ?= python
PROGRAMS_DIR := tests

# compiler flags. add -g for debug
FLAGS := -std=c99 -Wall -Wextra -O3 -fno-common -I. -Ivm -Iio

LDFLAGS :=

SRC  := $(wildcard *.c */*.c)
OBJS := $(SRC:.c=.o)
DEPS := $(OBJS:.o=.d)

ifeq ($(OS),Windows_NT)
TARGET := vm.exe
RUN    := .\$(TARGET)

# properly convert folder paths so make dont think theyre flags
WIN_CLEAN := $(subst /,\,$(OBJS) $(DEPS) $(TARGET))

clean:
	-@del /Q $(WIN_CLEAN) 2>NUL
	-@if exist $(PROGRAMS_DIR) rmdir /S /Q $(PROGRAMS_DIR)

else
TARGET := vm.out
RUN    := ./$(TARGET)

clean:
	$(RM) $(OBJS) $(DEPS) $(TARGET)
	$(RM) -r $(PROGRAMS_DIR)
endif

# default rm for non-windows
RM := rm -f

all: test $(TARGET)

test: clean $(TARGET)
	$(PYTHON) test.py
	$(PYTHON) runner.py

$(TARGET): $(OBJS)
	$(CC) $(OBJS) $(LDFLAGS) -o $@

%.o: %.c
	$(CC) $(FLAGS) -MMD -MP -c $< -o $@

-include $(DEPS)

run: all
	$(RUN)
