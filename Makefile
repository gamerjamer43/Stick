# all commands
.DEFAULT_GOAL := all
.PHONY: all clean run test

CC := gcc
PROGRAMS_DIR := tests

# compiler flags. add -g for debug
FLAGS := -std=c99 -Wall -Wextra -O3 -fno-common -I. -Ivm -Iio

# link time flags (which i didn't rly keep sep but i'll leave here)
LDFLAGS :=

SRC  := $(wildcard *.c */*.c)
OBJS := $(SRC:.c=.o)
DEPS := $(OBJS:.o=.d)

ifeq ($(OS),Windows_NT)
PYTHON := python
TARGET := vm.exe
RUNNER_ARGS :=
RUN    := .\$(TARGET)

# properly convert folder paths so make dont think theyre flags
WIN_CLEAN := $(subst /,\,$(OBJS) $(DEPS) $(TARGET))

clean:
	-@del /Q $(WIN_CLEAN) 2>NUL
	-@if exist $(PROGRAMS_DIR) rmdir /S /Q $(PROGRAMS_DIR)

else
RUNNER_ARGS := -p ./vm.out -c "valgrind --tool=memcheck --leak-check=full --show-leak-kinds=all --track-origins=yes --errors-for-leak-kinds=all --error-exitcode=1"
PYTHON := python3
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
	$(PYTHON) ./utils/test.py
	$(PYTHON) ./utils/runner.py $(RUNNER_ARGS)

$(TARGET): $(OBJS)
	$(CC) $(OBJS) $(LDFLAGS) -o $@

%.o: %.c
	$(CC) $(FLAGS) -MMD -MP -c $< -o $@

-include $(DEPS)

run: all
	$(RUN)
